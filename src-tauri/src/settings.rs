use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::AppError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub scoop_dir: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

pub fn legacy_active_root(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("settings.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("active_root").and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty()).map(|s| s.to_string())
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
    fn roundtrip_preserves_scoop_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Settings { scoop_dir: Some("D:\\ScoopUI\\scoop".into()), theme: None };
        save(tmp.path(), &s).unwrap();
        assert_eq!(load(tmp.path()).scoop_dir.as_deref(), Some("D:\\ScoopUI\\scoop"));
    }

    #[test]
    fn legacy_active_root_reads_old_field() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("settings.json"),
            r#"{"known_roots":["C:\\Users\\u\\scoop"],"active_root":"C:\\Users\\u\\scoop","theme":"dark"}"#,
        )
        .unwrap();
        assert_eq!(legacy_active_root(tmp.path()).as_deref(), Some("C:\\Users\\u\\scoop"));
        assert_eq!(load(tmp.path()).theme.as_deref(), Some("dark"));
    }

    #[test]
    fn legacy_active_root_ignores_missing_and_blank() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(legacy_active_root(tmp.path()).is_none());
        std::fs::write(tmp.path().join("settings.json"), r#"{"active_root":"   "}"#).unwrap();
        assert!(legacy_active_root(tmp.path()).is_none());
    }

    #[test]
    fn roundtrip_preserves_theme() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Settings { scoop_dir: None, theme: Some("dark".to_string()) };
        save(tmp.path(), &s).unwrap();
        assert_eq!(load(tmp.path()).theme.as_deref(), Some("dark"));
    }

    #[test]
    fn missing_or_corrupt_returns_default() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).scoop_dir.is_none());
        std::fs::write(tmp.path().join("settings.json"), "{not json").unwrap();
        assert!(load(tmp.path()).scoop_dir.is_none());
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
        assert!(loaded.scoop_dir.is_none());
        assert!(loaded.theme.is_none());
        let bak = std::fs::read_to_string(tmp.path().join("settings.json.bak")).unwrap();
        assert_eq!(bak, "{not json");
    }
}
