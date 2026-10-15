use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::domain::manifest;
use crate::domain::text;

/// Scoop 自身的安装目录（`apps\scoop`）不是用户应用，不参与列表与配置集比对。
pub const BUILTIN_APP: &str = "scoop";

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstalledApp {
    pub name: String,
    pub version: String,
    pub bucket: Option<String>,
    pub architecture: Option<String>,
    pub hold: bool,
    pub updated: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub icon: Option<String>,
    pub depends: Vec<String>,
    pub persist: Vec<String>,
    pub bin: Vec<String>,
    pub app_dir: String,
}

#[derive(Deserialize, Default)]
struct InstallJson {
    bucket: Option<String>,
    architecture: Option<String>,
    hold: Option<bool>,
    updated: Option<String>,
}

fn meta_path(dir: &Path, new_name: &str, old_name: &str) -> Option<PathBuf> {
    let new_path = dir.join(new_name);
    if new_path.is_file() {
        return Some(new_path);
    }
    let old_path = dir.join(old_name);
    if old_path.is_file() {
        return Some(old_path);
    }
    None
}

fn find_meta(current: &Path, new_name: &str, old_name: &str) -> Option<PathBuf> {
    meta_path(current, new_name, old_name)
}

/// 定位应用的清单目录。
///
/// 正常情况是 `apps\<名称>\current`（联接）。若 `current` 缺失或联接损坏，
/// 退化为「版本子目录中最新一个含清单的目录」，避免整条记录被丢弃。
fn locate_current(app_dir: &Path) -> Option<PathBuf> {
    let current = app_dir.join("current");
    if find_meta(&current, "scoop-manifest.json", "manifest.json").is_some() {
        return Some(current);
    }
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(app_dir).ok()?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if entry.file_name().to_string_lossy().eq_ignore_ascii_case("current") {
            continue;
        }
        if find_meta(&path, "scoop-manifest.json", "manifest.json").is_none() {
            continue;
        }
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        candidates.push((modified, path));
    }
    // 时间戳相同时用路径名兜底，保证选择结果可复现（取目录名较大者）。
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.pop().map(|(_, path)| path)
}

pub fn scan(apps_dir: &Path) -> Vec<InstalledApp> {
    let mut apps: Vec<InstalledApp> = Vec::new();
    let Ok(entries) = std::fs::read_dir(apps_dir) else {
        return apps;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.eq_ignore_ascii_case(BUILTIN_APP) {
            continue;
        }
        let Some(current) = locate_current(&dir) else {
            continue;
        };
        let Some(manifest_path) = find_meta(&current, "scoop-manifest.json", "manifest.json") else {
            continue;
        };
        let Some(manifest_text) = text::read_text(&manifest_path) else {
            continue;
        };
        let Ok(m) = manifest::parse(&manifest_text) else {
            continue;
        };
        // install 元数据缺失/损坏时不再丢弃整个应用，只退化为无软件源信息。
        let install: InstallJson = find_meta(&current, "scoop-install.json", "install.json")
            .and_then(|p| text::read_json(&p))
            .unwrap_or_default();
        apps.push(InstalledApp {
            name,
            version: m.version,
            bucket: install.bucket,
            architecture: install.architecture,
            hold: install.hold.unwrap_or(false),
            updated: install.updated,
            description: m.description,
            homepage: m.homepage,
            icon: m.icon,
            depends: m.depends,
            persist: m.persist,
            bin: m.bin,
            app_dir: dir.display().to_string(),
        });
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_app(root: &Path, name: &str, manifest: &str, install: Option<&str>) -> PathBuf {
        let current = root.join(name).join("current");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(current.join("manifest.json"), manifest).unwrap();
        if let Some(i) = install {
            std::fs::write(current.join("install.json"), i).unwrap();
        }
        current
    }

    #[test]
    fn scans_apps_with_manifest_and_optional_install() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = tmp.path();
        write_app(apps, "git", r#"{"version":"2.55.0.5","description":"git"}"#, Some(r#"{"bucket":"main","architecture":"64bit"}"#));
        write_app(apps, "7zip", r#"{"version":"26.00"}"#, Some(r#"{"bucket":"main"}"#));
        // 内置于 apps\scoop：不是用户应用
        write_app(apps, "scoop", r#"{"version":"0.5.3"}"#, None);
        // 缺 install.json：仍应被识别（bucket 未知）
        write_app(apps, "aria2", r#"{"version":"1.37.0"}"#, None);
        // 无清单：不是应用
        std::fs::create_dir_all(apps.join("orphan")).unwrap();

        let list = scan(apps);
        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["7zip", "aria2", "git"]);
        let git = list.iter().find(|a| a.name == "git").unwrap();
        assert_eq!(git.version, "2.55.0.5");
        assert_eq!(git.bucket.as_deref(), Some("main"));
        assert_eq!(git.architecture.as_deref(), Some("64bit"));
        assert!(!git.hold);
        let aria2 = list.iter().find(|a| a.name == "aria2").unwrap();
        assert_eq!(aria2.bucket, None);
        assert!(!aria2.hold);
    }

    #[test]
    fn skips_builtin_scoop_even_with_full_meta() {
        let tmp = tempfile::tempdir().unwrap();
        write_app(tmp.path(), "scoop", r#"{"version":"0.5.3"}"#, Some(r#"{"bucket":"main"}"#));
        write_app(tmp.path(), "Git", r#"{"version":"1.0"}"#, Some(r#"{"bucket":"main"}"#));
        let names: Vec<String> = scan(tmp.path()).into_iter().map(|a| a.name).collect();
        assert_eq!(names, vec!["Git"]);
    }

    #[test]
    fn falls_back_to_version_dir_when_current_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = tmp.path();
        // 两个版本目录，只有较新的含清单；`current` 联接缺失
        for version in ["1.0.0", "2.0.0"] {
            let dir = apps.join("git").join(version);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("manifest.json"), format!(r#"{{"version":"{version}"}}"#)).unwrap();
        }
        // 真实目录只剩一个 7-zip.dll 时不产生记录
        std::fs::create_dir_all(apps.join("broken").join("26.04")).unwrap();
        std::fs::write(apps.join("broken").join("26.04").join("7-zip.dll"), "x").unwrap();

        let list = scan(apps);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "git");
        assert_eq!(list[0].version, "2.0.0");
    }

    #[test]
    fn recognizes_manifest_with_bom() {
        let tmp = tempfile::tempdir().unwrap();
        let current = tmp.path().join("git").join("current");
        std::fs::create_dir_all(&current).unwrap();
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(r#"{"version":"2.55.0.5"}"#.as_bytes());
        std::fs::write(current.join("scoop-manifest.json"), bytes).unwrap();
        let mut install = vec![0xFF, 0xFE];
        for unit in r#"{"bucket":"main"}"#.encode_utf16() {
            install.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(current.join("scoop-install.json"), install).unwrap();

        let list = scan(tmp.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].version, "2.55.0.5");
        assert_eq!(list[0].bucket.as_deref(), Some("main"));
    }

    #[test]
    fn skips_app_with_unparsable_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write_app(tmp.path(), "weird", r#"{"description":"no version"}"#, Some("{}"));
        assert!(scan(tmp.path()).is_empty());
    }

    #[test]
    fn reads_hold_and_updated() {
        let tmp = tempfile::tempdir().unwrap();
        write_app(
            tmp.path(),
            "python",
            r#"{"version":"3.13.1","depends":["aria2"],"persist":"data"}"#,
            Some(r#"{"bucket":"main","hold":true,"updated":"2026-09-11T17:35:18Z"}"#),
        );
        let list = scan(tmp.path());
        assert_eq!(list.len(), 1);
        assert!(list[0].hold);
        assert_eq!(list[0].updated.as_deref(), Some("2026-09-11T17:35:18Z"));
        assert_eq!(list[0].depends, vec!["aria2"]);
        assert_eq!(list[0].persist, vec!["data"]);
    }

    #[test]
    fn missing_apps_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(scan(&tmp.path().join("nope")).is_empty());
    }

    #[test]
    fn scan_returns_apps_without_global_flag() {
        let tmp = tempfile::tempdir().unwrap();
        write_app(tmp.path(), "git", r#"{"version":"1.0"}"#, Some(r#"{"bucket":"main"}"#));
        let apps = scan(tmp.path());
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "git");
    }

    #[test]
    fn scan_reads_scoop_prefixed_meta_files() {
        let tmp = tempfile::tempdir().unwrap();
        let current = tmp.path().join("flutter").join("current");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(current.join("scoop-manifest.json"), r#"{"version":"3.47.7","description":"flutter"}"#).unwrap();
        std::fs::write(current.join("scoop-install.json"), r#"{"bucket":"extras","architecture":"64bit"}"#).unwrap();
        let apps = scan(tmp.path());
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "flutter");
        assert_eq!(apps[0].version, "3.47.7");
        assert_eq!(apps[0].bucket.as_deref(), Some("extras"));
    }

    #[test]
    fn scan_prefers_scoop_prefixed_when_both_present() {
        let tmp = tempfile::tempdir().unwrap();
        let current = tmp.path().join("git").join("current");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(current.join("manifest.json"), r#"{"version":"1.0"}"#).unwrap();
        std::fs::write(current.join("install.json"), r#"{"bucket":"old"}"#).unwrap();
        std::fs::write(current.join("scoop-manifest.json"), r#"{"version":"2.0"}"#).unwrap();
        std::fs::write(current.join("scoop-install.json"), r#"{"bucket":"new"}"#).unwrap();
        let apps = scan(tmp.path());
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].version, "2.0");
        assert_eq!(apps[0].bucket.as_deref(), Some("new"));
    }
}
