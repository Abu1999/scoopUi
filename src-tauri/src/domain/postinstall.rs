use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::AppError;

pub fn safe_app_name(name: &str) -> bool {
    !name.is_empty()
        && name == name.trim_end_matches([' ', '.'])
        && !name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control())
}

pub fn find_install_regs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if path.is_file() && name.starts_with("install-") && name.ends_with(".reg") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// 找出目录下的 `uninstall-*.reg`（应用自带的 shell 集成回滚脚本）。
pub fn find_uninstall_regs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if path.is_file() && name.starts_with("uninstall-") && name.ends_with(".reg") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// 应用内可能存放 `uninstall-*.reg` 的目录：应用根、`current`，以及各版本子目录。
///
/// 联接损坏、手工删过 `current` 或 `scoop uninstall` 中断时，回滚脚本只留在
/// 版本目录内；若漏扫则右键菜单/文件关联清理不干净。
pub fn uninstall_reg_dirs(app_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![app_dir.join("current"), app_dir.to_path_buf()];
    let Ok(entries) = std::fs::read_dir(app_dir) else {
        return dirs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !entry.file_name().to_string_lossy().eq_ignore_ascii_case("current") {
            dirs.push(path);
        }
    }
    dirs
}

/// 回放整个 Scoop 环境下所有应用的 `uninstall-*.reg`，返回成功条数。
///
/// 用于**删除环境/卸载应用之前**清掉右键菜单、文件关联等 shell 集成；
/// 直接删目录会留下「Git Bash Here」这类残留。
pub fn run_uninstall_regs(root: &Path) -> usize {
    let apps = root.join("apps");
    let Ok(entries) = std::fs::read_dir(&apps) else {
        return 0;
    };
    let mut done = 0usize;
    for app in entries.flatten() {
        if !app.path().is_dir() {
            continue;
        }
        for dir in uninstall_reg_dirs(&app.path()) {
            for reg in find_uninstall_regs(&dir) {
                let mut cmd = Command::new("reg.exe");
                crate::domain::proc::hide_window(&mut cmd);
                if cmd
                    .args(["import"])
                    .arg(&reg)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
                {
                    done += 1;
                }
            }
        }
    }
    done
}

pub fn register(dir: &Path, app_name: &str) -> Result<Vec<String>, AppError> {
    if !safe_app_name(app_name) {
        return Err(AppError::Other(format!("应用名称非法: {app_name}")));
    }
    if !dir.is_dir() {
        return Err(AppError::Other(format!("未找到已安装应用目录: {app_name}")));
    }
    let mut done: Vec<String> = Vec::new();
    for reg in find_install_regs(dir) {
        let mut cmd = Command::new("reg.exe");
        crate::domain::proc::hide_window(&mut cmd);
        let ok = cmd
            .args(["import"])
            .arg(&reg)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            done.push(reg.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
        }
    }
    if app_name.eq_ignore_ascii_case("git") {
        let git = dir.join("cmd").join("git.exe");
        if git.is_file() {
            let mut cmd = Command::new(&git);
            crate::domain::proc::hide_window(&mut cmd);
            let ok = cmd
                .args(["config", "--global", "credential.helper", "manager"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                done.push("credential.helper=manager".to_string());
            }
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_install_reg_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("install-context.reg"), "").unwrap();
        std::fs::write(tmp.path().join("install-associations.reg"), "").unwrap();
        std::fs::write(tmp.path().join("uninstall-context.reg"), "").unwrap();
        std::fs::write(tmp.path().join("readme.txt"), "").unwrap();
        let found = find_install_regs(tmp.path());
        assert_eq!(found.len(), 2);
        assert!(found
            .iter()
            .all(|p| p.file_name().unwrap().to_string_lossy().to_lowercase().starts_with("install-")));
        assert!(find_install_regs(&tmp.path().join("nope")).is_empty());
    }

    #[test]
    fn rejects_unsafe_names_and_missing_dir() {
        assert!(safe_app_name("git"));
        assert!(!safe_app_name("a/b"));
        assert!(!safe_app_name(".."));
        let tmp = tempfile::tempdir().unwrap();
        assert!(register(&tmp.path().join("nope"), "git").is_err());
        assert!(register(tmp.path(), "../x").is_err());
    }

    #[test]
    fn finds_only_uninstall_reg_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("uninstall-context-menu.reg"), "").unwrap();
        std::fs::write(tmp.path().join("uninstall-git.reg"), "").unwrap();
        std::fs::write(tmp.path().join("install-context.reg"), "").unwrap();
        std::fs::write(tmp.path().join("uninstall.reg"), "").unwrap();
        let found = find_uninstall_regs(tmp.path());
        assert_eq!(found.len(), 2);
        assert!(find_uninstall_regs(&tmp.path().join("nope")).is_empty());
    }

    #[test]
    fn uninstall_reg_dirs_cover_current_root_and_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("git");
        std::fs::create_dir_all(app.join("current")).unwrap();
        std::fs::create_dir_all(app.join("2.55.0")).unwrap();
        std::fs::write(app.join("readme.txt"), "x").unwrap();
        let dirs = uninstall_reg_dirs(&app);
        assert_eq!(dirs.len(), 3);
        assert_eq!(dirs[0], app.join("current"));
        assert_eq!(dirs[1], app);
        assert!(dirs.contains(&app.join("2.55.0")));
        // 目录不存在：返回基础两项且不 panic
        let missing = uninstall_reg_dirs(&app.join("nope"));
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn uninstall_regs_found_in_version_dir_when_current_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("git");
        let version = app.join("2.55.0");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(version.join("uninstall-context.reg"), "").unwrap();
        let found: Vec<_> = uninstall_reg_dirs(&app)
            .iter()
            .flat_map(|d| find_uninstall_regs(d))
            .collect();
        assert_eq!(found, vec![version.join("uninstall-context.reg")]);
    }

    #[test]
    fn run_uninstall_regs_scans_apps_and_skips_absent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        // 完全没有 apps 目录：不应报错，返回 0
        assert_eq!(run_uninstall_regs(tmp.path()), 0);
        // 有 apps 但应用目录里没有 reg：返回 0
        std::fs::create_dir_all(tmp.path().join("apps").join("7zip").join("current")).unwrap();
        assert_eq!(run_uninstall_regs(tmp.path()), 0);
    }
}
