use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::AppError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub known_roots: Vec<String>,
    #[serde(default)]
    pub active_root: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

pub fn load(dir: &Path) -> Settings {
    let file = dir.join("settings.json");
    match std::fs::read_to_string(&file) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(s) => s,
            Err(_) => {
                let _ = std::fs::copy(&file, dir.join("settings.json.bak"));
                Settings::default()
            }
        },
        Err(_) => Settings::default(),
    }
}

pub fn save(dir: &Path, s: &Settings) -> Result<(), AppError> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(s)?;
    let tmp = dir.join("settings.json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, dir.join("settings.json"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_values() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Settings {
            known_roots: vec!["C:\\scoop".into()],
            active_root: Some("C:\\scoop".into()),
            theme: None,
        };
        save(tmp.path(), &s).unwrap();
        let loaded = load(tmp.path());
        assert_eq!(loaded.known_roots, vec!["C:\\scoop".to_string()]);
        assert_eq!(loaded.active_root.as_deref(), Some("C:\\scoop"));
    }

    #[test]
    fn roundtrip_preserves_theme() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Settings {
            known_roots: vec![],
            active_root: None,
            theme: Some("dark".to_string()),
        };
        save(tmp.path(), &s).unwrap();
        assert_eq!(load(tmp.path()).theme.as_deref(), Some("dark"));
    }

    #[test]
    fn missing_or_corrupt_returns_default() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).known_roots.is_empty());
        std::fs::write(tmp.path().join("settings.json"), "{not json").unwrap();
        assert!(load(tmp.path()).active_root.is_none());
    }

    #[test]
    fn save_leaves_no_tmp_file() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), &Settings::default()).unwrap();
        assert!(tmp.path().join("settings.json").is_file());
        assert!(!tmp.path().join("settings.json.tmp").exists());
    }

    #[test]
    fn corrupt_file_is_backed_up_and_default_returned() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("settings.json"), "{not json").unwrap();
        let loaded = load(tmp.path());
        assert!(loaded.known_roots.is_empty());
        assert!(loaded.active_root.is_none());
        let bak = std::fs::read_to_string(tmp.path().join("settings.json.bak")).unwrap();
        assert_eq!(bak, "{not json");
    }
}
