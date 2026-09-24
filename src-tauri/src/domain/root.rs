use std::path::{Path, PathBuf};

use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct RootInfo {
    pub path: String,
    pub valid: bool,
    pub source: String,
    pub is_active: bool,
    pub app_count: usize,
    pub bucket_count: usize,
    pub has_portable_config: bool,
    pub global_dir: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: PathBuf,
    pub source: &'static str,
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

pub fn candidates(
    env_scoop: Option<PathBuf>,
    config_root_path: Option<PathBuf>,
    known: &[PathBuf],
    home: &Path,
) -> Vec<Candidate> {
    let mut list: Vec<Candidate> = Vec::new();
    if let Some(p) = env_scoop {
        list.push(Candidate { path: p, source: "env:SCOOP" });
    }
    if let Some(p) = config_root_path {
        list.push(Candidate { path: p, source: "config:root_path" });
    }
    for p in known {
        list.push(Candidate { path: p.clone(), source: "settings" });
    }
    list.push(Candidate { path: crate::paths::default_root(home), source: "default" });

    let mut seen: Vec<String> = Vec::new();
    list.into_iter()
        .filter(|c| {
            let key = normalized(&c.path);
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        })
        .collect()
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

pub fn info(path: &Path, source: &str, is_active: bool, global_dir: Option<&Path>) -> RootInfo {
    let valid = validate(path).is_ok();
    RootInfo {
        path: path.display().to_string(),
        valid,
        source: source.to_string(),
        is_active,
        app_count: if valid { crate::domain::installed::scan(&path.join("apps"), false).len() } else { 0 },
        bucket_count: if valid { count_dirs(&path.join("buckets")) } else { 0 },
        has_portable_config: path.join("config.json").is_file(),
        global_dir: global_dir.map(|g| g.display().to_string()),
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
    fn candidates_ordered_and_deduped_case_insensitive() {
        let list = candidates(
            Some(PathBuf::from("C:\\Scoop")),
            Some(PathBuf::from("D:\\scoop")),
            &[PathBuf::from("c:\\scoop"), PathBuf::from("E:\\scoop")],
            Path::new("C:\\Users\\u"),
        );
        let sources: Vec<&str> = list.iter().map(|c| c.source).collect();
        assert_eq!(sources, vec!["env:SCOOP", "config:root_path", "settings", "default"]);
        let paths: Vec<String> = list.iter().map(|c| c.path.display().to_string()).collect();
        assert_eq!(paths[2], "E:\\scoop");
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
    fn info_reports_counts_and_validity() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(tmp.path());
        let apps = root.join("apps");
        for (name, install) in [("git", Some(r#"{}"#)), ("scoop", None)] {
            let current = apps.join(name).join("current");
            std::fs::create_dir_all(&current).unwrap();
            std::fs::write(current.join("manifest.json"), r#"{"version":"1.0"}"#).unwrap();
            if let Some(i) = install {
                std::fs::write(current.join("install.json"), i).unwrap();
            }
        }
        std::fs::create_dir_all(root.join("buckets").join("main")).unwrap();
        let i = info(&root, "test", true, None);
        assert!(i.valid);
        assert!(i.is_active);
        assert_eq!(i.app_count, 1);
        assert_eq!(i.bucket_count, 1);
        let bad = info(&tmp.path().join("nope"), "test", false, None);
        assert!(!bad.valid);
        assert_eq!(bad.app_count, 0);
    }
}
