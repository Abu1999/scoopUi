#[cfg(windows)]
pub fn hide_window(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000);
}

#[cfg(not(windows))]
pub fn hide_window(_cmd: &mut std::process::Command) {}

/// 组装子进程 PATH：把 `dir` 前置；已存在（大小写与尾斜杠不敏感）则原样返回。
///
/// 应用进程的环境变量在启动时冻结：环境 shims 目录若是启动后才加入用户 PATH，
/// 子进程里看不到新装的 shim（如 git），表现为「装完必须重启应用」。
/// 调用方用它把 `<环境目录>\shims` 显式注入子进程 PATH。
pub fn merge_path_prepend(current: Option<&str>, dir: &str) -> String {
    let target = dir.trim_end_matches(['\\', '/']);
    let Some(current) = current.filter(|c| !c.trim().is_empty()) else {
        return dir.to_string();
    };
    let present = current.split(';').any(|entry| {
        entry
            .trim()
            .trim_end_matches(['\\', '/'])
            .eq_ignore_ascii_case(target)
    });
    if present {
        current.to_string()
    } else {
        format!("{dir};{current}")
    }
}

/// 重新拉起 Windows 资源管理器（结束它释放被加载的扩展 DLL 后恢复桌面/任务栏）。
///
/// 仅在已结束 explorer.exe 后调用；否则会变成打开一个文件夹窗口。
pub fn restart_explorer() {
    #[cfg(not(test))]
    {
        let sysroot = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        let mut exe = std::path::PathBuf::from(sysroot);
        exe.push("explorer.exe");
        let _ = std::process::Command::new(exe)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_path_prepend_rules() {
        assert_eq!(merge_path_prepend(None, r"D:\s\shims"), r"D:\s\shims");
        assert_eq!(merge_path_prepend(Some(""), r"D:\s\shims"), r"D:\s\shims");
        assert_eq!(merge_path_prepend(Some("  "), r"D:\s\shims"), r"D:\s\shims");
        assert_eq!(
            merge_path_prepend(Some(r"C:\a;C:\b"), r"D:\s\shims"),
            r"D:\s\shims;C:\a;C:\b"
        );
        // 已在其中（大小写/尾斜杠不敏感）：不重复前置
        assert_eq!(
            merge_path_prepend(Some(r"C:\a;D:\S\SHIMS\;C:\b"), r"D:\s\shims"),
            r"C:\a;D:\S\SHIMS\;C:\b"
        );
    }
}
