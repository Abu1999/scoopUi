use std::path::Path;

use serde::Serialize;

use crate::domain::manifest;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PackageEntry {
    pub name: String,
    pub bucket: String,
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub icon: Option<String>,
    pub bin: Vec<String>,
    pub deprecated: bool,
}

fn collect_json(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json(&path, out);
        } else if path.extension().map(|x| x.eq_ignore_ascii_case("json")).unwrap_or(false) {
            out.push(path);
        }
    }
}

pub fn build(buckets_dir: &Path) -> Vec<PackageEntry> {
    let mut entries: Vec<PackageEntry> = Vec::new();
    let Ok(buckets) = std::fs::read_dir(buckets_dir) else {
        return entries;
    };
    for bucket in buckets.flatten() {
        let bucket_dir = bucket.path();
        if !bucket_dir.is_dir() {
            continue;
        }
        let bucket_name = bucket.file_name().to_string_lossy().to_string();
        let mut files = Vec::new();
        collect_json(&bucket_dir.join("bucket"), &mut files);
        for file in files {
            let Some(name) = file.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            let Some(text) = crate::domain::text::read_text(&file) else {
                continue;
            };
            let Ok(m) = manifest::parse(&text) else {
                continue;
            };
            entries.push(PackageEntry {
                name,
                bucket: bucket_name.clone(),
                version: m.version,
                description: m.description,
                homepage: m.homepage,
                license: m.license,
                icon: m.icon,
                bin: m.bin,
                deprecated: m.deprecated,
            });
        }
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

pub fn search(entries: &[PackageEntry], query: &str, limit: usize) -> Vec<PackageEntry> {
    let q = query.trim().to_lowercase();
    let mut scored: Vec<(u8, &PackageEntry)> = entries
        .iter()
        .filter_map(|e| {
            let name = e.name.to_lowercase();
            let rank = if q.is_empty() {
                4
            } else if name == q {
                0
            } else if name.starts_with(&q) {
                1
            } else if name.contains(&q) {
                2
            } else if e.bucket.to_lowercase() == q {
                3
            } else if e
                .description
                .as_deref()
                .map(|d| d.to_lowercase().contains(&q))
                .unwrap_or(false)
            {
                4
            } else {
                return None;
            };
            Some((rank, e))
        })
        .collect();
    scored.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()))
    });
    scored.into_iter().take(limit).map(|(_, e)| e.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_manifest(dir: &std::path::Path, bucket: &str, name: &str, body: &str) {
        let b = dir.join(bucket).join("bucket");
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join(format!("{name}.json")), body).unwrap();
    }

    #[test]
    fn builds_entries_from_buckets() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(tmp.path(), "main", "git", r#"{"version":"2.55.0","description":"VCS","homepage":"https://gitforwindows.org"}"#);
        write_manifest(tmp.path(), "main", "sub", r#"{"version":"1.0","deprecated":true}"#);
        let sub = tmp.path().join("main").join("bucket").join("nested");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("deep.json"), r#"{"version":"9.9"}"#).unwrap();
        write_manifest(tmp.path(), "extras", "vscode", r#"{"version":"1.0"}"#);
        std::fs::write(
            tmp.path().join("main").join("bucket").join("broken.json"),
            "{not json",
        )
        .unwrap();

        let entries = build(tmp.path());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["deep", "git", "sub", "vscode"]);
        let git = entries.iter().find(|e| e.name == "git").unwrap();
        assert_eq!(git.bucket, "main");
        assert_eq!(git.version, "2.55.0");
        assert_eq!(git.description.as_deref(), Some("VCS"));
        let sub_e = entries.iter().find(|e| e.name == "sub").unwrap();
        assert!(sub_e.deprecated);
        assert_eq!(sub_e.bucket, "main");
    }

    #[test]
    fn missing_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(build(&tmp.path().join("nope")).is_empty());
    }

    #[test]
    fn search_ranks_and_limits() {
        let mk = |name: &str, desc: Option<&str>| PackageEntry {
            name: name.to_string(),
            bucket: "main".to_string(),
            version: "1".to_string(),
            description: desc.map(|s| s.to_string()),
            homepage: None,
            license: None,
            icon: None,
            bin: vec![],
            deprecated: false,
        };
        let entries = vec![
            mk("git", Some("distributed version control")),
            mk("git-lfs", Some("git extension")),
            mk("pylint", Some("a git hook tool")),
            mk("7zip", None),
        ];
        let r = search(&entries, "GIT", 10);
        let names: Vec<&str> = r.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["git", "git-lfs", "pylint"]);

        let r2 = search(&entries, "git", 2);
        assert_eq!(r2.len(), 2);
        assert_eq!(r2[0].name, "git");

        let r3 = search(&entries, "", 2);
        assert_eq!(r3.len(), 2);
        assert_eq!(r3[0].name, "7zip");

        assert!(search(&entries, "nomatchxyz", 10).is_empty());
    }

    #[test]
    fn search_matches_bucket_name_exactly() {
        let mk = |name: &str, bucket: &str, desc: Option<&str>| PackageEntry {
            name: name.to_string(),
            bucket: bucket.to_string(),
            version: "1".to_string(),
            description: desc.map(|s| s.to_string()),
            homepage: None,
            license: None,
            icon: None,
            bin: vec![],
            deprecated: false,
        };
        let entries = vec![
            mk("java-app", "extras", None),
            mk("temurin17-jdk", "java", Some("Eclipse Temurin runtime")),
            mk("zulu17-jdk", "java", Some("Zulu is certified build of OpenJDK")),
            mk("nodejs", "main", Some("JavaScript runtime")),
        ];
        let hits = search(&entries, "java", 10);
        let names: Vec<&str> = hits.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["java-app", "temurin17-jdk", "zulu17-jdk", "nodejs"]);
        let upper_hits = search(&entries, "JAVA", 10);
        let upper: Vec<&str> = upper_hits.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(upper, names);
    }

    #[test]
    fn bucket_match_is_exact_not_prefix() {
        let mk = |name: &str, bucket: &str| PackageEntry {
            name: name.to_string(),
            bucket: bucket.to_string(),
            version: "1".to_string(),
            description: None,
            homepage: None,
            license: None,
            icon: None,
            bin: vec![],
            deprecated: false,
        };
        let entries = vec![mk("zulu17-jdk", "java")];
        assert!(search(&entries, "javat", 10).is_empty());
    }

    #[test]
    fn collects_bin_from_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(
            tmp.path(),
            "main",
            "git",
            r#"{"version":"1.0","bin":[["git","git.exe"],"scalar.exe"]}"#,
        );
        let entries = build(tmp.path());
        assert_eq!(entries[0].bin, vec!["git", "scalar.exe"]);
    }
}
