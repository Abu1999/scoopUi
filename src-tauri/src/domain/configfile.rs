use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::error::AppError;

/// 解析环境配置文件：固定为环境目录内 `config.json`（Scoop 便携模式）。
///
/// 不再回退读写 `%USERPROFILE%\.config\scoop\config.json`——配置随环境目录
/// 同装同删；文件缺失时由启动/接入流程负责创建（`bootstrap::ensure_env_config`）。
pub fn resolve(root: &Path) -> PathBuf {
    root.join("config.json")
}

pub fn load(path: &Path) -> Value {
    crate::domain::text::read_text(path)
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| Value::Object(Default::default()))
}

pub fn set_key(path: &Path, key: &str, value: Option<Value>) -> Result<(), AppError> {
    let text = crate::domain::text::read_text(path);
    let trimmed = text.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let parsed = trimmed.and_then(|t| serde_json::from_str::<Value>(t).ok());
    if trimmed.is_some() && parsed.as_ref().map(|v| !v.is_object()).unwrap_or(true) {
        let _ = std::fs::copy(path, path.with_extension("json.bak"));
    }
    let mut current = match parsed {
        Some(Value::Object(map)) => Value::Object(map),
        _ => Value::Object(Default::default()),
    };
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
    fn resolves_to_env_config_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("scoop");
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(resolve(&root), root.join("config.json"));
        // 用户目录存在旧配置也不影响解析：只使用环境目录内文件
        let user = tmp.path().join(".config").join("scoop");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join("config.json"), "{}").unwrap();
        assert_eq!(resolve(&root), root.join("config.json"));
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

    #[test]
    fn set_key_backs_up_valid_non_object_json() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, "[1,2]").unwrap();
        set_key(&path, "proxy", Some(serde_json::json!("127.0.0.1:7890"))).unwrap();
        assert!(tmp.path().join("config.json.bak").is_file());
        assert_eq!(load(&path).get("proxy").and_then(|x| x.as_str()), Some("127.0.0.1:7890"));
    }

    #[test]
    fn set_key_keeps_object_file_without_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"scoop_branch":"master"}"#).unwrap();
        set_key(&path, "proxy", Some(serde_json::json!("127.0.0.1:7890"))).unwrap();
        assert!(!tmp.path().join("config.json.bak").is_file());
        assert_eq!(load(&path).get("scoop_branch").and_then(|x| x.as_str()), Some("master"));
    }
}
