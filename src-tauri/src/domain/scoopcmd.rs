use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::error::AppError;

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
        return Err(AppError::InvalidRoot(root.display().to_string()));
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

pub fn spawn(cmd: &ScoopCommand) -> Result<Child, AppError> {
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
    Ok(c.spawn()?)
}

pub fn stream_child(
    mut child: Child,
    mut on_line: impl FnMut(StreamLine) + Send,
) -> Result<Option<i32>, AppError> {
    let out = child.stdout.take();
    let err = child.stderr.take();
    let (tx, rx) = std::sync::mpsc::channel::<StreamLine>();

    let tx_out = tx.clone();
    let h_out = out.map(|o| {
        std::thread::spawn(move || {
            for line in BufReader::new(o).lines().map_while(Result::ok) {
                let _ = tx_out.send(StreamLine { stream: "stdout", line });
            }
        })
    });
    let h_err = err.map(|e| {
        std::thread::spawn(move || {
            for line in BufReader::new(e).lines().map_while(Result::ok) {
                let _ = tx.send(StreamLine { stream: "stderr", line });
            }
        })
    });

    let status = std::thread::scope(|s| {
        let drain = s.spawn(move || {
            for line in rx {
                on_line(line);
            }
        });
        let status = child.wait();
        let _ = drain.join();
        status
    })?;
    if let Some(h) = h_out {
        let _ = h.join();
    }
    if let Some(h) = h_err {
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
        assert!(build(tmp.path(), None, &["list".into()]).is_err());
    }

    #[test]
    fn streams_lines_and_exit_code() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "echo hello& echo warn 1>&2& exit /b 3".into()],
            envs: vec![],
            display: "test".into(),
        };
        let child = spawn(&cmd).unwrap();
        let mut lines: Vec<StreamLine> = Vec::new();
        let code = stream_child(child, |l| lines.push(l)).unwrap();
        assert_eq!(code, Some(3));
        assert!(lines.iter().any(|l| l.stream == "stdout" && l.line.contains("hello")));
        assert!(lines.iter().any(|l| l.stream == "stderr" && l.line.contains("warn")));
    }

    #[test]
    fn kill_tree_terminates_process() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "ping -n 30 127.0.0.1 >nul".into()],
            envs: vec![],
            display: "sleep".into(),
        };
        let mut child = spawn(&cmd).unwrap();
        let pid = child.id();
        assert!(kill_tree(pid));
        let start = std::time::Instant::now();
        let _ = child.wait();
        assert!(start.elapsed().as_secs() < 10);
    }
}
