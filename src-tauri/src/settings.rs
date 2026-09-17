use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::AppError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub known_roots: Vec<String>,
    #[serde(default)]
    pub active_root: Option<String>,
}

pub fn load(dir: &Path) -> Settings {
    let file = dir.join("settings.json");
    match std::fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(dir: &Path, s: &Settings) -> Result<(), AppError> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(s)?;
    std::fs::write(dir.join("settings.json"), text)?;
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
        };
        save(tmp.path(), &s).unwrap();
        let loaded = load(tmp.path());
        assert_eq!(loaded.known_roots, vec!["C:\\scoop".to_string()]);
        assert_eq!(loaded.active_root.as_deref(), Some("C:\\scoop"));
    }

    #[test]
    fn missing_or_corrupt_returns_default() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).known_roots.is_empty());
        std::fs::write(tmp.path().join("settings.json"), "{not json").unwrap();
        assert!(load(tmp.path()).active_root.is_none());
    }
}
