use serde::Deserialize;
use std::path::Path;

use crate::domain::manifest;

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
    pub global: bool,
}

#[derive(Deserialize, Default)]
struct InstallJson {
    bucket: Option<String>,
    architecture: Option<String>,
    hold: Option<bool>,
    updated: Option<String>,
}

pub fn scan(apps_dir: &Path, global: bool) -> Vec<InstalledApp> {
    let mut apps: Vec<InstalledApp> = Vec::new();
    let Ok(entries) = std::fs::read_dir(apps_dir) else {
        return apps;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let current = dir.join("current");
        let manifest_path = current.join("manifest.json");
        let install_path = current.join("install.json");
        if !manifest_path.is_file() || !install_path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(m) = manifest::parse(&text) else {
            continue;
        };
        let install: InstallJson = std::fs::read_to_string(&install_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        apps.push(InstalledApp {
            name: entry.file_name().to_string_lossy().to_string(),
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
            global,
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
    fn scans_only_dirs_with_manifest_and_install() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = tmp.path();
        write_app(apps, "git", r#"{"version":"2.55.0.5","description":"git"}"#, Some(r#"{"bucket":"main","architecture":"64bit"}"#));
        write_app(apps, "7zip", r#"{"version":"26.00"}"#, Some(r#"{"bucket":"main"}"#));
        write_app(apps, "scoop", r#"{"version":"0.5.3"}"#, None);
        std::fs::create_dir_all(apps.join("orphan")).unwrap();

        let list = scan(apps, false);
        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["7zip", "git"]);
        let git = &list[1];
        assert_eq!(git.version, "2.55.0.5");
        assert_eq!(git.bucket.as_deref(), Some("main"));
        assert_eq!(git.architecture.as_deref(), Some("64bit"));
        assert!(!git.hold);
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
        let list = scan(tmp.path(), false);
        assert_eq!(list.len(), 1);
        assert!(list[0].hold);
        assert_eq!(list[0].updated.as_deref(), Some("2026-09-11T17:35:18Z"));
        assert_eq!(list[0].depends, vec!["aria2"]);
        assert_eq!(list[0].persist, vec!["data"]);
    }

    #[test]
    fn missing_apps_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(scan(&tmp.path().join("nope"), false).is_empty());
    }

    #[test]
    fn marks_global_scan() {
        let tmp = tempfile::tempdir().unwrap();
        write_app(tmp.path(), "git", r#"{"version":"1.0"}"#, Some(r#"{"bucket":"main"}"#));
        let local = scan(tmp.path(), false);
        assert!(!local[0].global);
        let global = scan(tmp.path(), true);
        assert!(global[0].global);
    }
}
