use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;

pub const SETTINGS_WHITELIST: [&str; 5] = [
    "url_proxy", "proxy", "aria2-enabled", "aria2-split", "aria2-max-connection-per-server",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BucketRef {
    pub name: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileApp {
    pub name: String,
    pub bucket: Option<String>,
    #[serde(default)]
    pub global: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default = "default_version")]
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default, deserialize_with = "de_buckets")]
    pub buckets: Vec<BucketRef>,
    #[serde(default)]
    pub apps: Vec<ProfileApp>,
    #[serde(default)]
    pub settings: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileSummary {
    pub name: String,
    pub created: u64,
    pub bucket_count: usize,
    pub app_count: usize,
    pub settings_count: usize,
}

fn default_version() -> u32 {
    1
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BucketRefRaw {
    Name(String),
    Full(BucketRef),
}

fn de_buckets<'de, D>(deserializer: D) -> Result<Vec<BucketRef>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Vec::<BucketRefRaw>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|item| match item {
            BucketRefRaw::Name(name) => BucketRef { name, source: None },
            BucketRefRaw::Full(bucket) => bucket,
        })
        .collect())
}

pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn validate_profile_name(name: &str) -> Result<(), AppError> {
    let invalid = name.is_empty()
        || name.trim() != name
        || name == "."
        || name == ".."
        || name.ends_with('.')
        || name
            .chars()
            .any(|c| matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control());
    if invalid {
        return Err(AppError::Other(format!("配置集名称非法: {name}")));
    }
    Ok(())
}

fn profile_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

pub fn exists(dir: &Path, name: &str) -> bool {
    profile_path(dir, name).is_file()
}

pub fn list(dir: &Path) -> Vec<ProfileSummary> {
    let mut out: Vec<ProfileSummary> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
        {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(profile) = serde_json::from_str::<Profile>(&text) else {
            continue;
        };
        out.push(ProfileSummary {
            name: stem,
            created: profile.created,
            bucket_count: profile.buckets.len(),
            app_count: profile.apps.len(),
            settings_count: profile.settings.len(),
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn load(dir: &Path, name: &str) -> Result<Profile, AppError> {
    validate_profile_name(name)?;
    let text = std::fs::read_to_string(profile_path(dir, name))
        .map_err(|_| AppError::Other(format!("配置集不存在: {name}")))?;
    serde_json::from_str(&text).map_err(|_| AppError::Other(format!("配置集文件损坏: {name}")))
}

pub fn save(dir: &Path, profile: &Profile) -> Result<(), AppError> {
    validate_profile_name(&profile.name)?;
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(profile)?;
    let tmp = dir.join(format!("{}.json.tmp", profile.name));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, profile_path(dir, &profile.name))?;
    Ok(())
}

pub fn delete(dir: &Path, name: &str) -> Result<(), AppError> {
    validate_profile_name(name)?;
    let path = profile_path(dir, name);
    if !path.is_file() {
        return Err(AppError::Other(format!("配置集不存在: {name}")));
    }
    std::fs::remove_file(path)?;
    Ok(())
}

pub fn rename(dir: &Path, name: &str, new_name: &str) -> Result<(), AppError> {
    validate_profile_name(name)?;
    validate_profile_name(new_name)?;
    let mut profile = load(dir, name)?;
    if exists(dir, new_name) {
        return Err(AppError::Other(format!("配置集已存在: {new_name}")));
    }
    profile.name = new_name.to_string();
    save(dir, &profile)?;
    std::fs::remove_file(profile_path(dir, name))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Profile {
        Profile {
            version: 1,
            name: "工作".to_string(),
            created: 1758614400,
            buckets: vec![BucketRef {
                name: "main".to_string(),
                source: Some("https://github.com/ScoopInstaller/Main".to_string()),
            }],
            apps: vec![ProfileApp {
                name: "git".to_string(),
                bucket: Some("main".to_string()),
                global: false,
            }],
            settings: serde_json::json!({ "url_proxy": "https://scoop.201704.xyz" })
                .as_object()
                .unwrap()
                .clone(),
        }
    }

    #[test]
    fn validate_accepts_and_rejects() {
        assert!(validate_profile_name("工作").is_ok());
        assert!(validate_profile_name("my profile_2").is_ok());
        for bad in ["", ".", "..", " 工作", "工作 ", "工作.", "a/b", "a\\b", "C:\\x", "a:b", "a*b", "a\nb"] {
            assert!(validate_profile_name(bad).is_err(), "{bad:?} 应被拒绝");
        }
    }

    #[test]
    fn roundtrip_and_string_buckets() {
        let tmp = tempfile::tempdir().unwrap();
        let p = sample();
        save(tmp.path(), &p).unwrap();
        assert_eq!(load(tmp.path(), "工作").unwrap(), p);

        std::fs::write(
            tmp.path().join("混合.json"),
            r#"{"version":1,"name":"混合","buckets":["main",{"name":"extras","source":"https://e"}],"apps":[]}"#,
        )
        .unwrap();
        let mixed = load(tmp.path(), "混合").unwrap();
        assert_eq!(mixed.buckets.len(), 2);
        assert_eq!(mixed.buckets[0].source, None);
        assert_eq!(mixed.buckets[1].source.as_deref(), Some("https://e"));
    }

    #[test]
    fn list_skips_corrupt_and_sorts() {
        let tmp = tempfile::tempdir().unwrap();
        let mut a = sample();
        a.name = "alpha".to_string();
        save(tmp.path(), &a).unwrap();
        let mut b = sample();
        b.name = "Beta".to_string();
        save(tmp.path(), &b).unwrap();
        std::fs::write(tmp.path().join("broken.json"), "{oops").unwrap();
        let list = list(tmp.path());
        assert_eq!(list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["alpha", "Beta"]);
        assert_eq!(list[0].bucket_count, 1);
        assert_eq!(list[0].app_count, 1);
        assert_eq!(list[0].settings_count, 1);
    }

    #[test]
    fn save_load_delete_rename() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), &sample()).unwrap();
        assert!(exists(tmp.path(), "工作"));
        assert!(!exists(tmp.path(), "缺失"));
        assert!(load(tmp.path(), "缺失").is_err());

        let mut other = sample();
        other.name = "其他".to_string();
        save(tmp.path(), &other).unwrap();
        assert!(rename(tmp.path(), "工作", "其他").is_err());

        rename(tmp.path(), "工作", "新工作").unwrap();
        assert!(!exists(tmp.path(), "工作"));
        assert_eq!(load(tmp.path(), "新工作").unwrap().name, "新工作");

        delete(tmp.path(), "新工作").unwrap();
        assert!(delete(tmp.path(), "新工作").is_err());
    }

    #[test]
    fn save_leaves_no_tmp_file() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), &sample()).unwrap();
        assert!(tmp.path().join("工作.json").is_file());
        assert!(!tmp.path().join("工作.json.tmp").exists());
    }
}
