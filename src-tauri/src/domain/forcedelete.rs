//! 强制删除与「文件被占用」检测。
//!
//! Windows 上的「文件被占用」分两类：
//!
//! - **可删除占用**（打开者允许 `FILE_SHARE_DELETE`，如现代 Windows 加载的 DLL）：
//!   常规删除失败时可用 POSIX 语义删除（`SetFileInformationByHandle` +
//!   `FILE_DISPOSITION_FLAG_POSIX_SEMANTICS`）直接摘除目录项，无需关闭句柄；
//! - **不可删除占用**（打开者未共享删除权限）：必须先结束占用进程（`lock_holders`
//!   基于 Restart Manager 查询占用者），再常规删除。
//!
//! 删除环境 / 卸载应用的兜底清理都走这里，保证「卸载=删干净」。

use std::path::{Path, PathBuf};

/// 强制删除尝试的结果。
#[derive(Debug, Default)]
pub struct ForceDeleteReport {
    /// 是否已完整删除（无剩余条目）。
    pub removed: bool,
    /// 仍未能删除的条目（文件在前、目录在后；含各自失败原因不明）。
    pub remaining: Vec<PathBuf>,
}

/// 文件占用者（Restart Manager 查询结果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockHolder {
    pub pid: u32,
    pub name: String,
}

impl LockHolder {
    /// 是否是 Windows 资源管理器（结束它需要随后重新拉起，桌面会短暂闪烁）。
    pub fn is_explorer(&self) -> bool {
        self.name.to_ascii_lowercase().contains("explorer")
    }
}

/// 收集目录树中的文件与目录（目录按「子先父后」顺序，便于自底向上删除）。
///
/// 联接/符号链接不递归进入；按链接目标类型归类（通常 `current` 联接归入目录类）。
fn collect_tree(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    fn rec(dir: &Path, files: &mut Vec<PathBuf>, dirs: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                let is_dir = std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false);
                if is_dir {
                    dirs.push(path);
                } else {
                    files.push(path);
                }
            } else if meta.is_dir() {
                rec(&path, files, dirs);
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    rec(root, &mut files, &mut dirs);
    (files, dirs)
}

/// 以删除权限打开文件（共享读/写/删除）；失败说明占用者未共享删除权限。
#[cfg(windows)]
fn open_for_delete(path: &Path) -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, DELETE, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            DELETE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            None
        } else {
            Some(handle)
        }
    }
}

/// 能否以删除权限打开（能打开即可用 POSIX 语义删除）。
pub fn can_open_for_delete(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        if let Some(handle) = open_for_delete(path) {
            unsafe { CloseHandle(handle) };
            return true;
        }
        false
    }
    #[cfg(not(windows))]
    {
        path.is_file()
    }
}

/// 用 POSIX 语义删除单个文件：被加载的 DLL 等「共享删除权限」的占用也可立即摘除目录项。
pub fn posix_delete_file(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FileDispositionInfoEx, SetFileInformationByHandle, FILE_DISPOSITION_FLAG_DELETE,
            FILE_DISPOSITION_FLAG_POSIX_SEMANTICS, FILE_DISPOSITION_INFO_EX,
        };
        let Some(handle) = open_for_delete(path) else {
            return Err(std::io::Error::last_os_error());
        };
        let info = FILE_DISPOSITION_INFO_EX {
            Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
        };
        let ok = unsafe {
            SetFileInformationByHandle(
                handle,
                FileDispositionInfoEx,
                (&info as *const FILE_DISPOSITION_INFO_EX).cast(),
                std::mem::size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
            )
        };
        let err = if ok == 0 {
            Some(std::io::Error::last_os_error())
        } else {
            None
        };
        unsafe { CloseHandle(handle) };
        match err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_file(path)
    }
}

/// 探测目录树中「无法以删除权限打开」的文件（占用者未共享删除权限，须先结束进程），
/// 最多返回 `limit` 个。
pub fn blocked_files_under(root: &Path, limit: usize) -> Vec<PathBuf> {
    let (files, _) = collect_tree(root);
    let mut out = Vec::new();
    for file in files {
        if !can_open_for_delete(&file) {
            out.push(file);
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

/// 尽力删除整棵目录树：常规删除 → POSIX 语义删除兜底 → 自底向上删目录。
///
/// 返回仍未能删除的条目；`removed == true` 表示整棵树已删净。
pub fn remove_tree_force(root: &Path) -> ForceDeleteReport {
    if std::fs::symlink_metadata(root).is_err() {
        return ForceDeleteReport { removed: true, remaining: Vec::new() };
    }
    let (files, dirs) = collect_tree(root);
    let mut remaining: Vec<PathBuf> = Vec::new();
    for file in files {
        if std::fs::remove_file(&file).is_ok() {
            continue;
        }
        if posix_delete_file(&file).is_ok() {
            continue;
        }
        remaining.push(file);
    }
    for dir in dirs {
        if std::fs::remove_dir(&dir).is_ok() {
            continue;
        }
        remaining.push(dir);
    }
    if std::fs::remove_dir(root).is_err() && std::fs::symlink_metadata(root).is_ok() {
        remaining.push(root.to_path_buf());
    }
    ForceDeleteReport { removed: remaining.is_empty(), remaining }
}

#[cfg(windows)]
fn utf16_z(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// 查询占用这些文件的进程（Restart Manager），按 PID 去重。
pub fn lock_holders(files: &[PathBuf]) -> Vec<LockHolder> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
        use windows_sys::Win32::System::RestartManager::{
            RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
            RM_PROCESS_INFO,
        };
        if files.is_empty() {
            return Vec::new();
        }
        let mut holders: Vec<LockHolder> = Vec::new();
        let mut session: u32 = 0;
        let mut key: Vec<u16> = vec![0u16; CCH_RM_SESSION_KEY as usize + 1];
        unsafe {
            if RmStartSession(&mut session, 0, key.as_mut_ptr()) != 0 {
                return holders;
            }
            let wide_files: Vec<Vec<u16>> = files
                .iter()
                .map(|f| f.as_os_str().encode_wide().chain(std::iter::once(0)).collect())
                .collect();
            let ptrs: Vec<*const u16> = wide_files.iter().map(|w| w.as_ptr()).collect();
            let registered = RmRegisterResources(
                session,
                ptrs.len() as u32,
                ptrs.as_ptr(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
            ) == 0;
            if registered {
                let mut needed: u32 = 0;
                let mut listed: u32 = 0;
                let mut reasons: u32 = 0;
                let first =
                    RmGetList(session, &mut needed, &mut listed, std::ptr::null_mut(), &mut reasons);
                if first == ERROR_MORE_DATA && needed > 0 {
                    let mut infos: Vec<RM_PROCESS_INFO> =
                        vec![RM_PROCESS_INFO::default(); needed as usize];
                    let mut listed2 = needed;
                    if RmGetList(
                        session,
                        &mut needed,
                        &mut listed2,
                        infos.as_mut_ptr(),
                        &mut reasons,
                    ) == 0
                    {
                        for info in infos.iter().take(listed2 as usize) {
                            let pid = info.Process.dwProcessId;
                            if pid != 0 && !holders.iter().any(|h| h.pid == pid) {
                                holders.push(LockHolder { pid, name: utf16_z(&info.strAppName) });
                            }
                        }
                    }
                }
            }
            RmEndSession(session);
        }
        holders
    }
    #[cfg(not(windows))]
    {
        let _ = files;
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn wide(path: &Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// 以无共享模式打开（独占锁）：任何删除都应失败。
    #[cfg(windows)]
    unsafe fn open_exclusive(path: &Path) -> windows_sys::Win32::Foundation::HANDLE {
        use windows_sys::Win32::Foundation::GENERIC_READ;
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING,
        };
        CreateFileW(
            wide(path).as_ptr(),
            GENERIC_READ,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    }

    /// 以共享读/写/删除模式打开（可被删除的占用，模拟被加载的 DLL）。
    #[cfg(windows)]
    unsafe fn open_shared(path: &Path) -> windows_sys::Win32::Foundation::HANDLE {
        use windows_sys::Win32::Foundation::GENERIC_READ;
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        CreateFileW(
            wide(path).as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    }

    #[cfg(windows)]
    struct HandleGuard(windows_sys::Win32::Foundation::HANDLE);

    #[cfg(windows)]
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
        }
    }

    #[test]
    #[cfg(windows)]
    fn posix_delete_removes_file_held_with_shared_open() {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("loaded.dll");
        std::fs::write(&file, b"x").unwrap();
        let handle = unsafe { open_shared(&file) };
        assert_ne!(handle, INVALID_HANDLE_VALUE, "共享打开应成功");
        let _guard = HandleGuard(handle);

        posix_delete_file(&file).expect("POSIX 语义删除应成功");
        assert!(!file.exists(), "目录项应立即消失");
    }

    #[test]
    #[cfg(windows)]
    fn exclusive_lock_blocks_and_force_reports_remaining() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("app");
        let sub = root.join("1.0.0");
        std::fs::create_dir_all(&sub).unwrap();
        let locked = sub.join("7-zip.dll");
        std::fs::write(&locked, b"x").unwrap();
        std::fs::write(sub.join("readme.txt"), b"y").unwrap();

        let handle = unsafe { open_exclusive(&locked) };
        let guard = HandleGuard(handle);

        // 探测：锁定文件应被识别
        let blocked = blocked_files_under(&root, 10);
        assert_eq!(blocked, vec![locked.clone()]);

        // 强删：普通文件删除，锁定文件与目录保留
        let report = remove_tree_force(&root);
        assert!(!report.removed);
        assert!(report.remaining.contains(&locked));
        assert!(locked.exists());
        assert!(!sub.join("readme.txt").exists(), "未锁定文件应被删除");

        // 释放后强删应彻底成功
        drop(guard);
        let report2 = remove_tree_force(&root);
        assert!(report2.removed, "剩余: {:?}", report2.remaining);
        assert!(!root.exists());
    }

    #[test]
    fn remove_tree_force_missing_path_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let report = remove_tree_force(&tmp.path().join("nope"));
        assert!(report.removed);
        assert!(report.remaining.is_empty());
    }

    #[test]
    #[cfg(windows)]
    fn lock_holders_reports_process_holding_file() {
        use std::process::{Command, Stdio};
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("held.bin");
        std::fs::write(&file, b"x").unwrap();
        let script = format!(
            "$f=[IO.File]::Open('{}','Open','Read','None'); Start-Sleep -Seconds 30",
            file.display().to_string().replace('\'', "''")
        );
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let mut seen = false;
        for _ in 0..24 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if !can_open_for_delete(&file) {
                let holders = lock_holders(std::slice::from_ref(&file));
                if holders.iter().any(|h| h.pid == child.id()) {
                    seen = true;
                    break;
                }
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(seen, "Restart Manager 应报告持有独占句柄的进程（PID {}）", child.id());
    }

    /// 真机验证：把系统 DLL 复制到临时目录，由 PowerShell 子进程 `LoadLibrary` 占用（模拟
    /// 资源管理器加载 7-zip.dll）：
    ///
    /// - Restart Manager 能报出占用进程；
    /// - 被加载的 DLL 不共享删除权限，POSIX 语义删除也删不掉，必须先结束占用进程；
    /// - 结束占用后强删可清掉整棵目录。
    ///
    /// 运行：`cargo test --lib live_force_delete_loaded_dll -- --ignored --nocapture`
    #[test]
    #[ignore = "真实环境：需子进程加载 DLL；用 --ignored 运行"]
    #[cfg(windows)]
    fn live_force_delete_loaded_dll() {
        use std::process::{Command, Stdio};
        let sysroot = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        let src = std::path::PathBuf::from(sysroot).join("System32").join("version.dll");
        if !src.is_file() {
            eprintln!("skip: 未找到 {}", src.display());
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("app");
        let sub = root.join("1.0.0");
        std::fs::create_dir_all(&sub).unwrap();
        let dll = sub.join("7-zip.dll");
        std::fs::copy(&src, &dll).unwrap();

        let script = format!(
            "Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public static class N {{ [DllImport(\"kernel32.dll\")] public static extern System.IntPtr LoadLibrary(string p); }}'; \
             [N]::LoadLibrary('{}') | Out-Null; Start-Sleep -Seconds 30",
            dll.display().to_string().replace('\'', "''")
        );
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(4));

        let holders = lock_holders(std::slice::from_ref(&dll));
        eprintln!("loaded-dll holders: {holders:?}");
        eprintln!("can_open_for_delete(loaded)={}", can_open_for_delete(&dll));
        match posix_delete_file(&dll) {
            Ok(()) => eprintln!("posix_delete_file: OK"),
            Err(e) => eprintln!("posix_delete_file err: {e}"),
        }
        let before = remove_tree_force(&root);
        eprintln!("before kill: removed={} remaining={:?}", before.removed, before.remaining);

        let _ = child.kill();
        let _ = child.wait();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let after = remove_tree_force(&root);
        eprintln!("after kill: removed={} remaining={:?}", after.removed, after.remaining);

        assert!(holders.iter().any(|h| h.pid == child.id()), "应报出加载 DLL 的进程");
        assert!(after.removed, "结束占用后应能删净；剩余 {:?}", after.remaining);
        assert!(!root.exists());
    }
}
