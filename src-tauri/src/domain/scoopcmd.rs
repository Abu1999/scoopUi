use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::AppError;

const STREAM_GRACE: Duration = Duration::from_secs(3);
const STREAM_EXTRA_WAIT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoopCommand {
    pub program: String,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub display: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamLine {
    pub stream: &'static str,
    pub line: String,
}

pub fn quote_ps(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn build(root: &Path, global: Option<&Path>, args: &[String]) -> Result<ScoopCommand, AppError> {
    let ps1 = root.join("apps").join("scoop").join("current").join("bin").join("scoop.ps1");
    if !ps1.is_file() {
        return Err(AppError::MissingScoopScript(ps1.display().to_string()));
    }
    let joined = args.iter().map(|a| quote_ps(a)).collect::<Vec<_>>().join(" ");
    let script = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; $OutputEncoding=[Text.Encoding]::UTF8; & {} {}",
        quote_ps(&ps1.display().to_string()),
        joined
    );
    let mut envs = vec![("SCOOP".to_string(), root.display().to_string())];
    if let Some(g) = global {
        envs.push(("SCOOP_GLOBAL".to_string(), g.display().to_string()));
    }
    let display = format!("scoop {}", args.join(" "));
    Ok(ScoopCommand {
        program: "powershell.exe".to_string(),
        args: vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-Command".to_string(),
            script,
        ],
        envs,
        display,
    })
}

pub struct Spawned {
    pub child: Child,
    pub job: Option<JobHandle>,
}

impl Spawned {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

#[cfg(windows)]
pub struct JobHandle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for JobHandle {}

#[cfg(windows)]
impl JobHandle {
    fn create() -> Option<Self> {
        use windows_sys::Win32::System::JobObjects::CreateJobObjectW;
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            None
        } else {
            Some(Self(handle))
        }
    }

    fn assign(&self, child: &Child) -> bool {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as _) != 0 }
    }

    fn terminate(&self) {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(not(windows))]
pub struct JobHandle;

#[cfg(not(windows))]
impl JobHandle {
    fn terminate(&self) {}
}

pub fn spawn(cmd: &ScoopCommand) -> Result<Spawned, AppError> {
    let mut c = Command::new(&cmd.program);
    c.args(&cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &cmd.envs {
        c.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    let child = c.spawn()?;
    #[cfg(windows)]
    let job = {
        let mut job = JobHandle::create();
        if let Some(j) = job.as_ref() {
            if !j.assign(&child) {
                eprintln!("scoopcmd::spawn: 无法将子进程加入 Job Object，退化为直接等待；孙进程持有管道时可能延迟返回");
                job = None;
            }
        }
        job
    };
    #[cfg(not(windows))]
    let job: Option<JobHandle> = None;
    Ok(Spawned { child, job })
}

fn read_stream<R: std::io::Read>(
    reader: R,
    stream: &'static str,
    tx: std::sync::mpsc::Sender<StreamLine>,
) {
    let mut reader = BufReader::new(reader);
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                if buf.last() == Some(&b'\n') {
                    buf.pop();
                }
                if buf.last() == Some(&b'\r') {
                    buf.pop();
                }
                let line = String::from_utf8_lossy(&buf).into_owned();
                if tx.send(StreamLine { stream, line }).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn wait_threads(handles: &[std::thread::JoinHandle<()>], limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if handles.iter().all(|h| h.is_finished()) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

pub fn stream_child(
    spawned: Spawned,
    mut on_line: impl FnMut(StreamLine) + Send + 'static,
) -> Result<Option<i32>, AppError> {
    let Spawned { mut child, job } = spawned;
    let out = child.stdout.take();
    let err = child.stderr.take();
    let (tx, rx) = std::sync::mpsc::channel::<StreamLine>();

    let tx_out = tx.clone();
    let h_out = out.map(|o| std::thread::spawn(move || read_stream(o, "stdout", tx_out)));
    let h_err = err.map(|e| std::thread::spawn(move || read_stream(e, "stderr", tx)));

    let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let drain_stopped = stopped.clone();
    let drain = std::thread::spawn(move || {
        for line in rx {
            if drain_stopped.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            let hook = std::panic::AssertUnwindSafe(|| on_line(line));
            let _ = std::panic::catch_unwind(hook);
        }
    });

    let status = child.wait()?;

    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::with_capacity(3);
    if let Some(h) = h_out {
        handles.push(h);
    }
    if let Some(h) = h_err {
        handles.push(h);
    }
    handles.push(drain);

    if !wait_threads(&handles, STREAM_GRACE) {
        if let Some(j) = &job {
            j.terminate();
        }
        if !wait_threads(&handles, STREAM_EXTRA_WAIT) {
            stopped.store(true, std::sync::atomic::Ordering::SeqCst);
            eprintln!("scoopcmd::stream_child: 读取线程未在宽限期内结束，放弃等待并分离线程");
            return Ok(status.code());
        }
    }
    for h in handles {
        let _ = h.join();
    }
    Ok(status.code())
}

pub fn kill_tree(pid: u32) -> bool {
    let mut c = Command::new("taskkill.exe");
    c.args(["/T", "/F", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c.status().map(|s| s.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_root_with_ps1(base: &Path) -> std::path::PathBuf {
        let root = base.join("scoop");
        let bin = root.join("apps").join("scoop").join("current").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("scoop.ps1"), "# fake").unwrap();
        root
    }

    #[test]
    fn quotes_single_quotes_for_powershell() {
        assert_eq!(quote_ps("it's"), "'it''s'");
        assert_eq!(quote_ps("a b"), "'a b'");
    }

    #[test]
    fn build_sets_envs_and_script() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root_with_ps1(tmp.path());
        let cmd = build(
            &root,
            Some(Path::new("C:\\ProgramData\\scoop")),
            &["install".into(), "git".into(), "--global".into()],
        )
        .unwrap();
        assert_eq!(cmd.program, "powershell.exe");
        assert!(cmd.display.contains("scoop install git --global"));
        assert!(cmd.envs.contains(&("SCOOP".into(), root.display().to_string())));
        assert!(cmd.envs.contains(&("SCOOP_GLOBAL".into(), "C:\\ProgramData\\scoop".into())));
        let script = cmd.args.last().unwrap();
        assert!(script.contains("scoop.ps1"));
        assert!(script.contains("'install' 'git' '--global'"));
    }

    #[test]
    fn build_rejects_missing_ps1() {
        let tmp = tempfile::tempdir().unwrap();
        let err = build(tmp.path(), None, &["list".into()]).unwrap_err();
        assert!(matches!(err, AppError::MissingScoopScript(_)));
    }

    #[test]
    fn streams_lines_and_exit_code() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "echo hello& echo warn 1>&2& exit /b 3".into()],
            envs: vec![],
            display: "test".into(),
        };
        let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<StreamLine>::new()));
        let sink = lines.clone();
        let code = stream_child(spawn(&cmd).unwrap(), move |l| sink.lock().unwrap().push(l)).unwrap();
        let lines = lines.lock().unwrap();
        assert_eq!(code, Some(3));
        assert!(lines.iter().any(|l| l.stream == "stdout" && l.line.contains("hello")));
        assert!(lines.iter().any(|l| l.stream == "stderr" && l.line.contains("warn")));
    }

    #[test]
    fn returns_within_grace_when_grandchild_holds_pipes() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "start /B ping -n 60 127.0.0.1".into()],
            envs: vec![],
            display: "grandchild".into(),
        };
        let child = spawn(&cmd).unwrap();
        let start = std::time::Instant::now();
        let code = stream_child(child, |_| {}).unwrap();
        let elapsed = start.elapsed();
        println!("stream_child 耗时 {:?}", elapsed);
        assert_eq!(code, Some(0));
        assert!(elapsed.as_secs() < 15, "stream_child 耗时 {:?}", elapsed);
    }

    fn spawn_without_job(cmd: &ScoopCommand) -> Spawned {
        let mut spawned = spawn(cmd).unwrap();
        spawned.job = None;
        spawned
    }

    #[test]
    fn no_callbacks_after_detach() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "start /B ping -n 60 127.0.0.1".into()],
            envs: vec![],
            display: "detach".into(),
        };
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = count.clone();
        let started = std::time::Instant::now();
        let code = stream_child(spawn_without_job(&cmd), move |_| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
        .unwrap();
        let elapsed = started.elapsed();
        assert!(elapsed >= Duration::from_secs(4), "应走到 detach 分支: {:?}", elapsed);
        std::thread::sleep(Duration::from_millis(300));
        let at_quiet = count.load(std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(2500));
        let after = count.load(std::sync::atomic::Ordering::SeqCst);
        println!("detach 返回耗时 {:?}, 回调数 quiet={} after={}", elapsed, at_quiet, after);
        assert_eq!(
            after,
            at_quiet,
            "stream_child 返回后 on_line 仍在回调（{} 次）",
            after - at_quiet
        );
        assert_eq!(code, Some(0));
    }

    #[test]
    fn keeps_stream_after_invalid_utf8_bytes() {
        let script = "$s=[Console]::OpenStandardOutput(); $b=[byte[]](0xff,0xfe,10); $s.Write($b,0,$b.Length); $t=[byte[]](0x74,0x61,0x69,0x6c,10); $s.Write($t,0,$t.Length); $s.Flush()";
        let cmd = ScoopCommand {
            program: "powershell.exe".into(),
            args: vec!["-NoProfile".into(), "-Command".into(), script.into()],
            envs: vec![],
            display: "utf8".into(),
        };
        let child = spawn(&cmd).unwrap();
        let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<StreamLine>::new()));
        let sink = lines.clone();
        let code = stream_child(child, move |l| sink.lock().unwrap().push(l)).unwrap();
        assert_eq!(code, Some(0));
        let lines = lines.lock().unwrap();
        assert!(
            lines.iter().any(|l| l.stream == "stdout" && l.line.contains('\u{fffd}')),
            "应收到非法字节行（有损解码）: {:?}",
            lines.iter().map(|l| l.line.as_str()).collect::<Vec<_>>()
        );
        assert!(lines.iter().any(|l| l.line == "tail"), "非法 UTF-8 之后的行不应丢失");
    }

    #[test]
    fn callback_panic_does_not_abort_stream() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "echo one& echo two& exit /b 5".into()],
            envs: vec![],
            display: "panic".into(),
        };
        let child = spawn(&cmd).unwrap();
        let lines = std::sync::Arc::new(std::sync::Mutex::new(Vec::<StreamLine>::new()));
        let sink = lines.clone();
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = seen.clone();
        let code = stream_child(child, move |l| {
            let index = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            sink.lock().unwrap().push(l);
            if index == 0 {
                panic!("模拟回调 panic");
            }
        })
        .unwrap();
        assert_eq!(code, Some(5));
        assert_eq!(lines.lock().unwrap().len(), 2);
    }

    #[test]
    fn kill_tree_terminates_process() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "ping -n 30 127.0.0.1 >nul".into()],
            envs: vec![],
            display: "sleep".into(),
        };
        let mut spawned = spawn(&cmd).unwrap();
        assert!(kill_tree(spawned.pid()));
        let start = std::time::Instant::now();
        let _ = spawned.child.wait();
        assert!(start.elapsed().as_secs() < 10);
    }
}
