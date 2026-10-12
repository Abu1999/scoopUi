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

pub fn register(dir: &Path, app_name: &str) -> Result<Vec<String>, AppError> {
    if !safe_app_name(app_name) {
        return Err(AppError::Other(format!("应用名称非法: {app_name}")));
    }
    if !dir.is_dir() {
        return Err(AppError::Other(format!("未找到已安装应用目录: {app_name}")));
    }
    let mut done: Vec<String> = Vec::new();
    for reg in find_install_regs(dir) {
        let ok = Command::new("reg.exe")
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
            let ok = Command::new(&git)
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
}
