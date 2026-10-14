use std::cmp::Ordering;

use serde::Serialize;

use crate::domain::index::PackageEntry;
use crate::domain::installed::InstalledApp;
use crate::domain::versions::compare_versions;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpdateInfo {
    pub name: String,
    pub current: String,
    pub latest: String,
    pub bucket: String,
    pub hold: bool,
}

pub fn compute(installed: &[InstalledApp], entries: &[PackageEntry]) -> Vec<UpdateInfo> {
    let mut out: Vec<UpdateInfo> = Vec::new();
    for app in installed {
        let mut candidates: Vec<&PackageEntry> = entries
            .iter()
            .filter(|e| e.name.eq_ignore_ascii_case(&app.name))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        candidates.sort_by(|a, b| a.bucket.to_lowercase().cmp(&b.bucket.to_lowercase()));
        let chosen = app
            .bucket
            .as_deref()
            .and_then(|b| candidates.iter().find(|e| e.bucket.eq_ignore_ascii_case(b)).copied())
            .unwrap_or(candidates[0]);
        if compare_versions(&app.version, &chosen.version) == Ordering::Less {
            out.push(UpdateInfo {
                name: app.name.clone(),
                current: app.version.clone(),
                latest: chosen.version.clone(),
                bucket: chosen.bucket.clone(),
                hold: app.hold,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::index::PackageEntry;
    use crate::domain::installed::InstalledApp;

    fn app(name: &str, version: &str, bucket: Option<&str>, hold: bool) -> InstalledApp {
        InstalledApp {
            name: name.to_string(),
            version: version.to_string(),
            bucket: bucket.map(|s| s.to_string()),
            architecture: None,
            hold,
            updated: None,
            description: None,
            homepage: None,
            icon: None,
            depends: vec![],
            persist: vec![],
            bin: vec![],
            app_dir: format!("C:\\scoop\\apps\\{name}"),
        }
    }

    fn entry(name: &str, bucket: &str, version: &str) -> PackageEntry {
        PackageEntry {
            name: name.to_string(),
            bucket: bucket.to_string(),
            version: version.to_string(),
            description: None,
            homepage: None,
            license: None,
            icon: None,
            bin: vec![],
            deprecated: false,
        }
    }

    #[test]
    fn detects_newer_bucket_version() {
        let apps = vec![app("git", "2.53.0.2", Some("main"), false)];
        let entries = vec![entry("git", "main", "2.55.0.5")];
        let out = compute(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "git");
        assert_eq!(out[0].current, "2.53.0.2");
        assert_eq!(out[0].latest, "2.55.0.5");
        assert_eq!(out[0].hold, false);
    }

    #[test]
    fn no_update_when_equal_or_newer_installed() {
        let apps = vec![
            app("7zip", "26.00", Some("main"), false),
            app("python", "3.14.0", Some("main"), false),
        ];
        let entries = vec![entry("7zip", "main", "26.00"), entry("python", "main", "3.13.1")];
        assert!(compute(&apps, &entries).is_empty());
    }

    #[test]
    fn hold_app_still_listed_with_flag() {
        let apps = vec![app("git", "1.0", Some("main"), true)];
        let entries = vec![entry("git", "main", "2.0")];
        let out = compute(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert!(out[0].hold);
    }

    #[test]
    fn prefers_installed_bucket_and_matches_name_case_insensitively() {
        let apps = vec![app("Git", "1.0", Some("versions"), false)];
        let entries = vec![
            entry("git", "main", "9.0"),
            entry("git", "versions", "2.0"),
        ];
        let out = compute(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].bucket, "versions");
        assert_eq!(out[0].latest, "2.0");
    }

    #[test]
    fn missing_bucket_match_falls_back_to_first_sorted() {
        let apps = vec![app("git", "1.0", Some("gone"), false)];
        let entries = vec![entry("git", "zzz", "3.0"), entry("git", "aaa", "2.0")];
        let out = compute(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].bucket, "aaa");
    }

    #[test]
    fn sorted_by_name_and_skips_unknown() {
        let apps = vec![
            app("zeta", "1.0", Some("main"), false),
            app("alpha", "1.0", Some("main"), false),
            app("unknown", "1.0", Some("main"), false),
        ];
        let entries = vec![entry("zeta", "main", "2.0"), entry("alpha", "main", "2.0")];
        let out = compute(&apps, &entries);
        let names: Vec<&str> = out.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }
}
