use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::buckets::BucketInfo;
use crate::domain::installed::InstalledApp;
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

fn is_remote_url(source: &str) -> bool {
    let s = source.trim().to_ascii_lowercase();
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("git@") || s.starts_with("ssh://")
}

fn validate_setting(key: &str, value: &Value) -> Result<(), AppError> {
    let ok = match key {
        "url_proxy" | "proxy" => value.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false),
        "aria2-enabled" => value.is_boolean(),
        "aria2-split" | "aria2-max-connection-per-server" => value.as_u64().is_some(),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(AppError::Other(format!("settings.{key} 类型不符")))
    }
}

fn whitelist_from(config: &Value) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    let Some(obj) = config.as_object() else {
        return map;
    };
    for key in SETTINGS_WHITELIST {
        let found = obj
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v);
        if let Some(v) = found {
            if validate_setting(key, v).is_ok() {
                map.insert(key.to_string(), v.clone());
            }
        }
    }
    map
}

pub fn snapshot(
    name: &str,
    buckets: &[BucketInfo],
    apps: &[InstalledApp],
    config: &Value,
) -> Profile {
    Profile {
        version: 1,
        name: name.to_string(),
        created: now_epoch(),
        buckets: buckets
            .iter()
            .map(|b| BucketRef {
                name: b.name.clone(),
                source: b.source.clone(),
            })
            .collect(),
        apps: apps
            .iter()
            .map(|a| ProfileApp {
                name: a.name.clone(),
                bucket: a.bucket.clone(),
                global: a.global,
            })
            .collect(),
        settings: whitelist_from(config),
    }
}

#[derive(Deserialize)]
struct ScoopExport {
    #[serde(default)]
    buckets: Vec<ScoopBucket>,
    #[serde(default)]
    apps: Vec<ScoopApp>,
    #[serde(default)]
    config: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
struct ScoopBucket {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Source", default)]
    source: Option<String>,
}

#[derive(Deserialize)]
struct ScoopApp {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Source", default)]
    source: Option<String>,
    #[serde(rename = "Info", default)]
    info: Option<String>,
}

pub fn parse_import(text: &str, name: &str) -> Result<Profile, AppError> {
    let value: Value =
        serde_json::from_str(text).map_err(|_| AppError::Other("无法识别的配置集文件".to_string()))?;
    if let Ok(profile) = serde_json::from_value::<Profile>(value.clone()) {
        let mut p = profile;
        p.name = name.to_string();
        return Ok(p);
    }
    let export: ScoopExport =
        serde_json::from_value(value).map_err(|_| AppError::Other("无法识别的配置集文件".to_string()))?;
    if export.apps.is_empty() && export.buckets.is_empty() {
        return Err(AppError::Other("无法识别的配置集文件".to_string()));
    }
    let buckets = export
        .buckets
        .into_iter()
        .map(|b| BucketRef {
            name: b.name,
            source: b.source.filter(|s| is_remote_url(s)),
        })
        .collect();
    let apps = export
        .apps
        .into_iter()
        .map(|a| {
            let global = a
                .info
                .as_deref()
                .map(|i| i.split(", ").any(|p| p == "Global install"))
                .unwrap_or(false);
            let bucket = a
                .source
                .filter(|s| !s.trim().is_empty() && s != "<auto-generated>" && !s.contains("://"));
            ProfileApp {
                name: a.name,
                bucket,
                global,
            }
        })
        .collect();
    Ok(Profile {
        version: 1,
        name: name.to_string(),
        created: now_epoch(),
        buckets,
        apps,
        settings: whitelist_from(&Value::Object(export.config)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::buckets::BucketInfo;
    use crate::domain::installed::InstalledApp;

    fn installed_app(name: &str, bucket: Option<&str>, global: bool) -> InstalledApp {
        InstalledApp {
            name: name.to_string(),
            version: "1.0".to_string(),
            bucket: bucket.map(|s| s.to_string()),
            architecture: None,
            hold: false,
            updated: None,
            description: None,
            homepage: None,
            icon: None,
            depends: vec![],
            persist: vec![],
            bin: vec![],
            app_dir: format!("C:\\scoop\\apps\\{name}"),
            global,
        }
    }

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

    #[test]
    fn snapshot_collects_buckets_apps_and_whitelist() {
        let buckets = vec![BucketInfo {
            name: "main".to_string(),
            source: Some("https://github.com/ScoopInstaller/Main".to_string()),
            is_git: true,
            manifest_count: 3,
        }];
        let apps = vec![
            installed_app("git", Some("main"), false),
            installed_app("7zip", None, true),
        ];
        let config = serde_json::json!({
            "url_proxy": "https://scoop.201704.xyz",
            "proxy": "",
            "aria2-enabled": false,
            "aria2-split": 5,
            "other_key": 1
        });
        let p = snapshot("工作", &buckets, &apps, &config);
        assert_eq!(p.version, 1);
        assert_eq!(p.name, "工作");
        assert!(p.created > 0);
        assert_eq!(p.buckets.len(), 1);
        assert_eq!(p.buckets[0].source.as_deref(), Some("https://github.com/ScoopInstaller/Main"));
        assert_eq!(p.apps.len(), 2);
        assert!(!p.apps[0].global);
        assert!(p.apps[1].global);
        assert_eq!(p.settings.get("url_proxy").and_then(|v| v.as_str()), Some("https://scoop.201704.xyz"));
        assert_eq!(p.settings.get("aria2-enabled").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(p.settings.get("aria2-split").and_then(|v| v.as_u64()), Some(5));
        assert!(!p.settings.contains_key("proxy"));
        assert!(!p.settings.contains_key("other_key"));
    }

    #[test]
    fn parse_import_reads_own_format_and_overrides_name() {
        let text = r#"{"version":1,"name":"旧名","created":1,"buckets":["main"],"apps":[{"name":"git","bucket":"main","global":false}],"settings":{}}"#;
        let p = parse_import(text, "新名").unwrap();
        assert_eq!(p.name, "新名");
        assert_eq!(p.buckets[0].name, "main");
        assert_eq!(p.apps[0].name, "git");
    }

    #[test]
    fn parse_import_converts_scoop_export() {
        let text = r#"{
          "buckets": [
            {"Name":"main","Source":"https://github.com/ScoopInstaller/Main"},
            {"Name":"local","Source":"C:\\mybucket"}
          ],
          "apps": [
            {"Name":"git","Source":"main","Info":"Held package, 64bit"},
            {"Name":"python","Source":"<auto-generated>","Info":"Global install"},
            {"Name":"tool","Source":"https://example.com/tool.json","Info":""}
          ],
          "config": {"url_proxy":"https://scoop.201704.xyz","root_path":"C:\\scoop","aria2-enabled":true}
        }"#;
        let p = parse_import(text, "导入").unwrap();
        assert_eq!(p.buckets.len(), 2);
        assert_eq!(p.buckets[0].source.as_deref(), Some("https://github.com/ScoopInstaller/Main"));
        assert_eq!(p.buckets[1].source, None);
        assert_eq!(p.apps.len(), 3);
        assert_eq!(p.apps[0].bucket.as_deref(), Some("main"));
        assert!(!p.apps[0].global);
        assert!(p.apps[1].global);
        assert_eq!(p.apps[1].bucket, None);
        assert_eq!(p.apps[2].bucket, None);
        assert_eq!(p.settings.get("url_proxy").and_then(|v| v.as_str()), Some("https://scoop.201704.xyz"));
        assert_eq!(p.settings.get("aria2-enabled").and_then(|v| v.as_bool()), Some(true));
        assert!(!p.settings.contains_key("root_path"));
    }

    #[test]
    fn parse_import_rejects_unknown() {
        assert!(parse_import("{not json", "x").is_err());
        assert!(parse_import("{}", "x").is_err());
        assert!(parse_import(r#"{"buckets":[],"apps":[]}"#, "x").is_err());
    }
}
