use serde::Serialize;

use crate::domain::index::PackageEntry;
use crate::domain::registry::RegistryApp;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MigrationMatch {
    pub name: String,
    pub bucket: String,
    pub confidence: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MigrationEntry {
    pub app: RegistryApp,
    pub matches: Vec<MigrationMatch>,
}

pub fn normalize_name(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_lowercase())
        .collect()
}

fn bin_stems(entry: &PackageEntry) -> Vec<String> {
    entry
        .bin
        .iter()
        .map(|b| {
            let file = b.rsplit(['\\', '/']).next().unwrap_or(b);
            file.trim_end_matches(".exe")
                .trim_end_matches(".EXE")
                .to_lowercase()
        })
        .collect()
}

fn rank(confidence: &str) -> u8 {
    match confidence {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    }
}

pub fn match_apps(apps: &[RegistryApp], entries: &[PackageEntry]) -> Vec<MigrationEntry> {
    let mut out: Vec<MigrationEntry> = apps
        .iter()
        .map(|app| {
            let normalized = normalize_name(&app.name);
            let name_words = words(&app.name);
            let mut matches: Vec<MigrationMatch> = Vec::new();
            for entry in entries {
                let confidence = if !normalized.is_empty() && normalize_name(&entry.name) == normalized {
                    "high"
                } else if app.exe_names.iter().any(|e| bin_stems(entry).iter().any(|b| b == e)) {
                    "medium"
                } else {
                    let desc_words = entry.description.as_deref().map(words).unwrap_or_default();
                    let overlap = name_words.iter().filter(|w| desc_words.contains(w)).count();
                    if overlap >= 2 {
                        "low"
                    } else {
                        continue;
                    }
                };
                matches.push(MigrationMatch {
                    name: entry.name.clone(),
                    bucket: entry.bucket.clone(),
                    confidence: confidence.to_string(),
                    version: entry.version.clone(),
                    description: entry.description.clone(),
                });
            }
            matches.sort_by(|a, b| {
                rank(&a.confidence)
                    .cmp(&rank(&b.confidence))
                    .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            matches.truncate(5);
            MigrationEntry {
                app: app.clone(),
                matches,
            }
        })
        .collect();
    out.sort_by(|a, b| a.app.name.to_lowercase().cmp(&b.app.name.to_lowercase()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::index::PackageEntry;
    use crate::domain::registry::RegistryApp;

    fn raw_app(name: &str, exe: &[&str], managed: bool) -> RegistryApp {
        RegistryApp {
            key_path: r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\X".to_string(),
            name: name.to_string(),
            version: Some("1.0".to_string()),
            publisher: None,
            install_location: None,
            uninstall_string: None,
            exe_names: exe.iter().map(|s| s.to_string()).collect(),
            managed,
        }
    }

    fn entry(name: &str, desc: Option<&str>, bin: &[&str]) -> PackageEntry {
        PackageEntry {
            name: name.to_string(),
            bucket: "main".to_string(),
            version: "1".to_string(),
            description: desc.map(|s| s.to_string()),
            homepage: None,
            license: None,
            icon: None,
            bin: bin.iter().map(|s| s.to_string()).collect(),
            deprecated: false,
        }
    }

    #[test]
    fn normalize_strips_punctuation_and_case() {
        assert_eq!(normalize_name("Visual-Studio  Code!"), "visualstudiocode");
        assert_eq!(normalize_name("7-Zip"), "7zip");
    }

    #[test]
    fn high_confidence_on_normalized_exact_name() {
        let apps = vec![raw_app("Git", &[], false)];
        let entries = vec![entry("git", Some("Distributed VCS"), &["git"])];
        let out = match_apps(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].matches.len(), 1);
        assert_eq!(out[0].matches[0].confidence, "high");
        assert_eq!(out[0].matches[0].bucket, "main");
    }

    #[test]
    fn medium_confidence_on_bin_match() {
        let apps = vec![raw_app("Git for Windows", &["git"], false)];
        let entries = vec![entry("git", Some("distributed version control"), &["git.exe"])];
        let out = match_apps(&apps, &entries);
        assert_eq!(out[0].matches[0].confidence, "medium");
    }

    #[test]
    fn low_confidence_on_description_word_overlap() {
        let apps = vec![raw_app("Adobe Acrobat Reader DC", &[], false)];
        let entries = vec![entry(
            "pdf-reader",
            Some("Read and annotate Adobe Acrobat documents"),
            &[],
        )];
        let out = match_apps(&apps, &entries);
        assert_eq!(out[0].matches.len(), 1);
        assert_eq!(out[0].matches[0].confidence, "low");
    }

    #[test]
    fn no_match_and_managed_passthrough() {
        let apps = vec![raw_app("Totally Unrelated", &[], true)];
        let entries = vec![entry("git", None, &[])];
        let out = match_apps(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert!(out[0].matches.is_empty());
        assert!(out[0].app.managed);
    }

    #[test]
    fn sorts_by_confidence_then_name_and_limits() {
        let apps = vec![raw_app("Git Tools Suite", &["scalar"], false)];
        let entries = vec![
            entry("gittoolssuite", Some("all-in-one bundle"), &[]),
            entry("git-lfs", Some("git large file storage"), &["scalar.exe"]),
            entry("git-tools", Some("suite of git tools"), &[]),
        ];
        let out = match_apps(&apps, &entries);
        let confidences: Vec<&str> = out[0].matches.iter().map(|m| m.confidence.as_str()).collect();
        assert_eq!(confidences, vec!["high", "medium", "low"]);
        let names: Vec<&str> = out[0].matches.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["gittoolssuite", "git-lfs", "git-tools"]);
    }
}
