use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::buckets::BucketInfo;
use crate::domain::installed::InstalledApp;
use crate::domain::profiles::{is_remote_url, now_epoch, BucketRef};
use crate::error::AppError;

pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub version: u32,
    pub created: u64,
    pub root: String,
    pub config_path: String,
    pub config_present: bool,
    pub app_count: usize,
    pub bucket_count: usize,
    pub profile_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotApp {
    pub name: String,
    pub bucket: Option<String>,
    pub version: String,
    #[serde(default)]
    pub hold: bool,
    #[serde(default)]
    pub global: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SnapshotSummary {
    pub id: String,
    pub created: u64,
    pub root: String,
    pub app_count: usize,
    pub bucket_count: usize,
    pub profile_count: usize,
}

pub fn validate_id(id: &str) -> Result<(), AppError> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Other(format!("快照 ID 非法: {id}")));
    }
    Ok(())
}

pub fn snapshot_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(id)
}

pub fn list(dir: &Path) -> Vec<SnapshotSummary> {
    let mut out: Vec<SnapshotSummary> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let Ok(meta) = load_meta(dir, &id) else {
            continue;
        };
        out.push(SnapshotSummary {
            id,
            created: meta.created,
            root: meta.root,
            app_count: meta.app_count,
            bucket_count: meta.bucket_count,
            profile_count: meta.profile_count,
        });
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

pub fn load_meta(dir: &Path, id: &str) -> Result<SnapshotMeta, AppError> {
    validate_id(id)?;
    let text = std::fs::read_to_string(snapshot_dir(dir, id).join("meta.json"))
        .map_err(|_| AppError::Other(format!("快照不存在: {id}")))?;
    serde_json::from_str(&text).map_err(|_| AppError::Other(format!("快照文件损坏: {id}")))
}

pub fn load_buckets(dir: &Path, id: &str) -> Result<Vec<BucketRef>, AppError> {
    load_json(dir, id, "buckets.json")
}

pub fn load_apps(dir: &Path, id: &str) -> Result<Vec<SnapshotApp>, AppError> {
    load_json(dir, id, "apps.json")
}

fn load_json<T: serde::de::DeserializeOwned>(dir: &Path, id: &str, file: &str) -> Result<T, AppError> {
    validate_id(id)?;
    let text = std::fs::read_to_string(snapshot_dir(dir, id).join(file))
        .map_err(|_| AppError::Other(format!("快照不存在: {id}")))?;
    serde_json::from_str(&text).map_err(|_| AppError::Other(format!("快照文件损坏: {id}")))
}

pub fn create(
    dir: &Path,
    root: &Path,
    config_path: &Path,
    buckets: &[BucketInfo],
    apps: &[InstalledApp],
    profiles_src: &Path,
) -> Result<SnapshotSummary, AppError> {
    std::fs::create_dir_all(dir)?;
    let mut created = now_epoch();
    let mut id = created.to_string();
    while snapshot_dir(dir, &id).exists() {
        created += 1;
        id = created.to_string();
    }
    let target = snapshot_dir(dir, &id);
    std::fs::create_dir_all(&target)?;

    let config_present = config_path.is_file();
    if config_present {
        std::fs::copy(config_path, target.join("config.json"))?;
    }
    let bucket_refs: Vec<BucketRef> = buckets
        .iter()
        .map(|b| BucketRef {
            name: b.name.clone(),
            source: b.source.clone(),
        })
        .collect();
    std::fs::write(target.join("buckets.json"), serde_json::to_string_pretty(&bucket_refs)?)?;
    let snapshot_apps: Vec<SnapshotApp> = apps
        .iter()
        .map(|a| SnapshotApp {
            name: a.name.clone(),
            bucket: a.bucket.clone(),
            version: a.version.clone(),
            hold: a.hold,
            global: a.global,
        })
        .collect();
    std::fs::write(target.join("apps.json"), serde_json::to_string_pretty(&snapshot_apps)?)?;

    let mut profile_count = 0usize;
    if profiles_src.is_dir() {
        let dst = target.join("profiles");
        std::fs::create_dir_all(&dst)?;
        if let Ok(entries) = std::fs::read_dir(profiles_src) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_json = path
                    .extension()
                    .map(|e| e.eq_ignore_ascii_case("json"))
                    .unwrap_or(false);
                if path.is_file() && is_json && std::fs::copy(&path, dst.join(entry.file_name())).is_ok() {
                    profile_count += 1;
                }
            }
        }
    }

    let meta = SnapshotMeta {
        version: SNAPSHOT_VERSION,
        created,
        root: root.display().to_string(),
        config_path: config_path.display().to_string(),
        config_present,
        app_count: apps.len(),
        bucket_count: buckets.len(),
        profile_count,
    };
    std::fs::write(target.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

    Ok(SnapshotSummary {
        id,
        created,
        root: meta.root,
        app_count: meta.app_count,
        bucket_count: meta.bucket_count,
        profile_count: meta.profile_count,
    })
}

pub fn delete(dir: &Path, id: &str) -> Result<(), AppError> {
    validate_id(id)?;
    let target = snapshot_dir(dir, id);
    if !target.is_dir() {
        return Err(AppError::Other(format!("快照不存在: {id}")));
    }
    std::fs::remove_dir_all(target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::buckets::BucketInfo;
    use crate::domain::installed::InstalledApp;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn bucket(name: &str, source: Option<&str>) -> BucketInfo {
        BucketInfo {
            name: name.to_string(),
            source: source.map(|s| s.to_string()),
            is_git: source.is_some(),
            manifest_count: 0,
        }
    }

    fn app(name: &str, bucket: Option<&str>, version: &str, global: bool) -> InstalledApp {
        InstalledApp {
            name: name.to_string(),
            version: version.to_string(),
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

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let t = tmp();
        let root = t.path().join("scoop");
        std::fs::create_dir_all(root.join("buckets")).unwrap();
        let config = root.join("config.json");
        std::fs::write(&config, r#"{"url_proxy":"https://scoop.201704.xyz"}"#).unwrap();
        let profiles = t.path().join("profiles");
        std::fs::create_dir_all(&profiles).unwrap();
        std::fs::write(profiles.join("工作.json"), "{}").unwrap();
        (t, root, config)
    }

    #[test]
    fn validate_id_rejects_non_numeric() {
        assert!(validate_id("1758614400").is_ok());
        for bad in ["", "..", "a1", "1/2", "1 2"] {
            assert!(validate_id(bad).is_err(), "{bad:?} 应被拒绝");
        }
    }

    #[test]
    fn create_writes_all_files_and_meta_last() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let profiles = t.path().join("profiles");
        let summary = create(
            &dir,
            &root,
            &config,
            &[bucket("main", Some("https://github.com/ScoopInstaller/Main"))],
            &[app("git", Some("main"), "2.55.0.5", false), app("7zip", Some("main"), "26.00", true)],
            &profiles,
        )
        .unwrap();
        assert!(summary.id.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(summary.app_count, 2);
        assert_eq!(summary.bucket_count, 1);
        assert_eq!(summary.profile_count, 1);
        assert!(summary.created > 0);
        let target = snapshot_dir(&dir, &summary.id);
        for f in ["meta.json", "config.json", "buckets.json", "apps.json"] {
            assert!(target.join(f).is_file(), "{f} 应存在");
        }
        assert!(target.join("profiles").join("工作.json").is_file());
        let meta = load_meta(&dir, &summary.id).unwrap();
        assert!(meta.config_present);
        assert_eq!(meta.root, root.display().to_string());

        let buckets = load_buckets(&dir, &summary.id).unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].source.as_deref(), Some("https://github.com/ScoopInstaller/Main"));
        let apps = load_apps(&dir, &summary.id).unwrap();
        assert_eq!(apps.len(), 2);
        assert!(apps.iter().any(|a| a.name == "7zip" && a.global));
    }

    #[test]
    fn list_skips_corrupt_and_sorts_desc() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let a = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        let b = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        assert_ne!(a.id, b.id);
        std::fs::create_dir_all(dir.join("not-numeric")).unwrap();
        std::fs::write(dir.join("not-numeric").join("meta.json"), "{}").unwrap();
        let list = list(&dir);
        assert_eq!(list.len(), 2);
        assert!(list[0].created >= list[1].created);
    }

    #[test]
    fn delete_removes_dir_and_missing_errors() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        delete(&dir, &s.id).unwrap();
        assert!(!snapshot_dir(&dir, &s.id).exists());
        assert!(delete(&dir, &s.id).is_err());
        assert!(load_meta(&dir, &s.id).is_err());
    }

    #[test]
    fn create_without_config_or_profiles_is_ok() {
        let (t, root, _config) = fixture();
        let dir = t.path().join("snapshots");
        let summary = create(&dir, &root, &root.join("missing.json"), &[], &[], &t.path().join("nope")).unwrap();
        assert_eq!(summary.profile_count, 0);
        let meta = load_meta(&dir, &summary.id).unwrap();
        assert!(!meta.config_present);
        assert!(!snapshot_dir(&dir, &summary.id).join("config.json").exists());
    }
}
