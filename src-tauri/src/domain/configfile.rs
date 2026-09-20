use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::error::AppError;

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

pub fn set_key(path: &Path, key: &str, value: Option<Value>) -> Result<(), AppError> {
    let mut current = load(path);
    if !current.is_object() {
        current = Value::Object(Default::default());
    }
    if let Ok(text) = std::fs::read_to_string(path) {
        let trimmed = text.trim();
        if !trimmed.is_empty() && serde_json::from_str::<Value>(trimmed).is_err() {
            let _ = std::fs::copy(path, path.with_extension("json.bak"));
        }
    }
    let obj = current.as_object_mut().expect("上面已保证是对象");
    let entries = std::mem::take(obj);
    for (k, v) in entries {
        obj.insert(k.to_ascii_lowercase(), v);
    }
    match value {
        Some(v) => {
            obj.insert(key.to_ascii_lowercase(), v);
        }
        None => {
            obj.remove(&key.to_ascii_lowercase());
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&current)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
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

    #[test]
    fn set_key_preserves_other_keys_and_removes_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"scoop_branch":"master","Keep":1}"#).unwrap();
        set_key(&path, "URL_PROXY", Some(serde_json::json!("https://x"))).unwrap();
        let v = load(&path);
        assert_eq!(v.get("url_proxy").and_then(|x| x.as_str()), Some("https://x"));
        assert_eq!(v.get("scoop_branch").and_then(|x| x.as_str()), Some("master"));
        assert_eq!(v.get("keep").and_then(|x| x.as_i64()), Some(1));
        set_key(&path, "url_proxy", None).unwrap();
        assert!(load(&path).get("url_proxy").is_none());
    }

    #[test]
    fn set_key_backs_up_corrupt_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, "{oops").unwrap();
        set_key(&path, "proxy", Some(serde_json::json!("127.0.0.1:7890"))).unwrap();
        assert!(tmp.path().join("config.json.bak").is_file());
        assert_eq!(load(&path).get("proxy").and_then(|x| x.as_str()), Some("127.0.0.1:7890"));
    }
}
