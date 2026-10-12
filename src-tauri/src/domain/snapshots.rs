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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestorePlan {
    pub root: String,
    pub root_matches: bool,
    pub config_path: String,
    pub config_present: bool,
    pub reset_buckets: Vec<BucketRef>,
    pub missing_buckets: Vec<BucketRef>,
    pub missing_apps: Vec<SnapshotApp>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestoreBucketsResult {
    pub reset: Vec<String>,
    pub missing: Vec<BucketRef>,
    pub skipped: Vec<String>,
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

pub fn plan(
    dir: &Path,
    id: &str,
    current_root: &Path,
    current_config_path: &Path,
    current_buckets: &[BucketInfo],
    current_apps: &[InstalledApp],
) -> Result<RestorePlan, AppError> {
    let meta = load_meta(dir, id)?;
    let mut buckets = load_buckets(dir, id)?;
    let mut apps = load_apps(dir, id)?;
    let lower = |s: &str| s.to_lowercase();
    let current_names: Vec<String> = current_buckets.iter().map(|b| lower(&b.name)).collect();
    let mut reset_buckets: Vec<BucketRef> = Vec::new();
    let mut missing_buckets: Vec<BucketRef> = Vec::new();
    buckets.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    for b in buckets {
        if current_names.contains(&lower(&b.name)) {
            if b.source.as_deref().map(is_remote_url).unwrap_or(false) {
                reset_buckets.push(b);
            }
        } else {
            missing_buckets.push(b);
        }
    }
    let app_key = |name: &str, global: bool| (lower(name), global);
    let current_keys: Vec<(String, bool)> = current_apps.iter().map(|a| app_key(&a.name, a.global)).collect();
    apps.retain(|a| !current_keys.contains(&app_key(&a.name, a.global)));
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.global.cmp(&b.global))
    });
    Ok(RestorePlan {
        root: meta.root.clone(),
        root_matches: crate::domain::root::normalized(current_root)
            == crate::domain::root::normalized(Path::new(&meta.root)),
        config_path: current_config_path.display().to_string(),
        config_present: meta.config_present,
        reset_buckets,
        missing_buckets,
        missing_apps: apps,
    })
}

pub fn restore_config(dir: &Path, id: &str, config_path: &Path) -> Result<(), AppError> {
    let meta = load_meta(dir, id)?;
    if !meta.config_present {
        return Err(AppError::Other("该快照未包含配置文件".to_string()));
    }
    let text = std::fs::read_to_string(snapshot_dir(dir, id).join("config.json"))
        .map_err(|_| AppError::Other(format!("快照文件损坏: {id}")))?;
    serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|_| AppError::Other(format!("快照文件损坏: {id}")))?;
    if config_path.is_file() {
        let _ = std::fs::copy(config_path, config_path.with_extension("json.bak"));
    }
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = config_path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, config_path)?;
    Ok(())
}

fn safe_bucket_name(name: &str) -> bool {
    !name.is_empty()
        && name == name.trim_end_matches([' ', '.'])
        && !name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control())
}

pub fn restore_buckets(dir: &Path, id: &str, root: &Path) -> Result<RestoreBucketsResult, AppError> {
    let buckets = load_buckets(dir, id)?;
    let mut reset: Vec<String> = Vec::new();
    let mut missing: Vec<BucketRef> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for b in buckets {
        if !safe_bucket_name(&b.name) {
            skipped.push(b.name);
            continue;
        }
        let local = root.join("buckets").join(&b.name);
        if !local.is_dir() {
            missing.push(b);
            continue;
        }
        let Some(source) = b.source.as_deref().filter(|s| is_remote_url(s)) else {
            skipped.push(b.name);
            continue;
        };
        if crate::domain::buckets::git_remote(&local).is_none() {
            skipped.push(b.name);
            continue;
        }
        crate::domain::mirrors::run_git(
            &local.display().to_string(),
            &["remote", "set-url", "origin", source],
        )?;
        reset.push(b.name);
    }
    Ok(RestoreBucketsResult { reset, missing, skipped })
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

    #[test]
    fn plan_reports_missing_apps_buckets_and_reset() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(
            &dir,
            &root,
            &config,
            &[
                bucket("main", Some("https://github.com/ScoopInstaller/Main")),
                bucket("extras", Some("https://gitee.com/scoop-installer/Extras")),
            ],
            &[
                app("git", Some("main"), "2.55.0.5", false),
                app("7zip", Some("main"), "26.00", true),
            ],
            &t.path().join("none"),
        )
        .unwrap();
        let current_buckets = vec![
            bucket("main", Some("https://github.com/ScoopInstaller/Main")),
            bucket("java", None),
        ];
        let current_apps = vec![app("git", Some("main"), "2.55.0.5", false)];
        let p = plan(&dir, &s.id, &root, &config, &current_buckets, &current_apps).unwrap();
        assert!(p.root_matches);
        assert!(p.config_present);
        assert_eq!(p.config_path, config.display().to_string());
        assert_eq!(p.reset_buckets.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["main"]);
        assert_eq!(p.missing_buckets.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["extras"]);
        assert_eq!(p.missing_apps.len(), 1);
        assert!(p.missing_apps[0].global);
        assert_eq!(p.missing_apps[0].name, "7zip");
    }

    #[test]
    fn plan_flags_root_mismatch() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        let p = plan(&dir, &s.id, &t.path().join("other"), &config, &[], &[]).unwrap();
        assert!(!p.root_matches);
    }

    #[test]
    fn restore_config_backs_up_and_writes() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        std::fs::write(&config, r#"{"old":true}"#).unwrap();
        restore_config(&dir, &s.id, &config).unwrap();
        let restored = std::fs::read_to_string(&config).unwrap();
        assert!(restored.contains("url_proxy"));
        let bak = std::fs::read_to_string(root.join("config.json.bak")).unwrap();
        assert!(bak.contains("old"));
        assert!(!root.join("config.json.tmp").exists());
    }

    #[test]
    fn restore_config_without_config_errors() {
        let (t, root, _config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(&dir, &root, &root.join("missing.json"), &[], &[], &t.path().join("none")).unwrap();
        let err = restore_config(&dir, &s.id, &root.join("config.json")).unwrap_err().to_string();
        assert!(err.contains("未包含配置文件"), "实际: {err}");
    }

    #[test]
    fn restore_buckets_sets_remote_and_reports_missing() {
        let (t, root, config) = fixture();
        let main = root.join("buckets").join("main");
        std::fs::create_dir_all(&main).unwrap();
        crate::domain::mirrors::run_git(&main.display().to_string(), &["init", "-q"]).unwrap();
        crate::domain::mirrors::run_git(
            &main.display().to_string(),
            &["remote", "add", "origin", "https://github.com/ScoopInstaller/Main"],
        )
        .unwrap();
        std::fs::create_dir_all(root.join("buckets").join("notgit")).unwrap();

        let dir = t.path().join("snapshots");
        let s = create(
            &dir,
            &root,
            &config,
            &[
                bucket("main", Some("https://gitee.com/scoop-installer/Main")),
                bucket("extras", Some("https://gitee.com/scoop-installer/Extras")),
                bucket("notgit", Some("https://gitee.com/scoop-installer/NotGit")),
                bucket("localonly", None),
            ],
            &[],
            &t.path().join("none"),
        )
        .unwrap();
        let result = restore_buckets(&dir, &s.id, &root).unwrap();
        assert_eq!(result.reset, vec!["main"]);
        assert_eq!(
            result.missing.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            vec!["extras", "localonly"]
        );
        assert_eq!(result.skipped, vec!["notgit"]);
        assert_eq!(
            crate::domain::buckets::git_remote(&main).as_deref(),
            Some("https://gitee.com/scoop-installer/Main")
        );
    }
}
