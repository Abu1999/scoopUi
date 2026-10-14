use std::path::{Path, PathBuf};

use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct RootInfo {
    pub path: String,
    pub valid: bool,
    pub app_count: usize,
    pub bucket_count: usize,
    pub has_portable_config: bool,
}

pub fn validate(path: &Path) -> Result<(), AppError> {
    for sub in ["apps", "buckets", "shims"] {
        if !path.join(sub).is_dir() {
            return Err(AppError::InvalidRoot(path.display().to_string()));
        }
    }
    Ok(())
}

fn count_dirs(p: &Path) -> usize {
    match std::fs::read_dir(p) {
        Ok(entries) => entries.flatten().filter(|e| e.path().is_dir()).count(),
        Err(_) => 0,
    }
}

pub fn normalized(p: &Path) -> String {
    p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase()
}

pub fn global_dir(
    env_global: Option<PathBuf>,
    config_global_path: Option<PathBuf>,
    program_data: &Path,
) -> PathBuf {
    env_global
        .or(config_global_path)
        .unwrap_or_else(|| crate::paths::default_global_root(program_data))
}

pub fn info(path: &Path) -> RootInfo {
    let valid = validate(path).is_ok();
    RootInfo {
        path: path.display().to_string(),
        valid,
        app_count: if valid { crate::domain::installed::scan(&path.join("apps")).len() } else { 0 },
        bucket_count: if valid { count_dirs(&path.join("buckets")) } else { 0 },
        has_portable_config: path.join("config.json").is_file(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_root(base: &Path) -> PathBuf {
        let root = base.join("scoop");
        for sub in ["apps", "buckets", "shims"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        root
    }

    #[test]
    fn validate_requires_three_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(tmp.path());
        assert!(validate(&root).is_ok());
        std::fs::remove_dir(root.join("shims")).unwrap();
        assert!(validate(&root).is_err());
    }

    #[test]
    fn global_dir_precedence() {
        let pd = Path::new("C:\\ProgramData");
        assert_eq!(
            global_dir(Some(PathBuf::from("X:\\g")), Some(PathBuf::from("Y:\\g")), pd),
            PathBuf::from("X:\\g")
        );
        assert_eq!(
            global_dir(None, Some(PathBuf::from("Y:\\g")), pd),
            PathBuf::from("Y:\\g")
        );
        assert_eq!(global_dir(None, None, pd), PathBuf::from("C:\\ProgramData\\scoop"));
    }

    #[test]
    fn info_reports_counts_without_global() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(tmp.path());
        std::fs::create_dir_all(root.join("apps").join("git").join("current")).unwrap();
        std::fs::write(root.join("apps").join("git").join("current").join("manifest.json"), r#"{"version":"1.0"}"#).unwrap();
        std::fs::write(root.join("apps").join("git").join("current").join("install.json"), r#"{"bucket":"main"}"#).unwrap();
        let i = info(&root);
        assert!(i.valid);
        assert_eq!(i.app_count, 1);
        assert_eq!(i.bucket_count, 0);
        let bad = info(&tmp.path().join("nope"));
        assert!(!bad.valid);
        assert_eq!(bad.app_count, 0);
    }
}
