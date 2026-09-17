use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub portable: bool,
}

pub fn resolve(root: &Path, user_config_home: &Path) -> ConfigFile {
    let portable = root.join("config.json");
    if portable.is_file() {
        ConfigFile { path: portable, portable: true }
    } else {
        ConfigFile {
            path: user_config_home.join("scoop").join("config.json"),
            portable: false,
        }
    }
}

pub fn load(path: &Path) -> Value {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| Value::Object(Default::default())),
        Err(_) => Value::Object(Default::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_portable_config() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("scoop");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("config.json"), "{}").unwrap();
        let c = resolve(&root, tmp.path());
        assert!(c.portable);
        assert_eq!(c.path, root.join("config.json"));
    }

    #[test]
    fn falls_back_to_user_config() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("scoop");
        std::fs::create_dir_all(&root).unwrap();
        let c = resolve(&root, tmp.path());
        assert!(!c.portable);
        assert_eq!(c.path, tmp.path().join("scoop").join("config.json"));
    }

    #[test]
    fn load_missing_or_corrupt_returns_empty_object() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(load(&tmp.path().join("nope.json")), Value::Object(Default::default()));
        std::fs::write(tmp.path().join("bad.json"), "{oops").unwrap();
        assert_eq!(load(&tmp.path().join("bad.json")), Value::Object(Default::default()));
    }
}
