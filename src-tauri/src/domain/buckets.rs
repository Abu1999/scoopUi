use std::path::Path;

use crate::domain::text;

#[derive(Debug, Clone, serde::Serialize)]
pub struct BucketInfo {
    pub name: String,
    pub source: Option<String>,
    pub is_git: bool,
    pub manifest_count: usize,
}

fn git_config_path(bucket_dir: &Path) -> Option<std::path::PathBuf> {
    let git = bucket_dir.join(".git");
    if git.is_dir() {
        Some(git.join("config"))
    } else if git.is_file() {
        let text = text::read_text(&git)?;
        let line = text.lines().find(|l| l.trim_start().starts_with("gitdir:"))?;
        let dir = line.trim_start().trim_start_matches("gitdir:").trim();
        let p = std::path::PathBuf::from(dir);
        let p = if p.is_absolute() { p } else { bucket_dir.join(p) };
        Some(p.join("config"))
    } else {
        None
    }
}

pub fn git_remote(bucket_dir: &Path) -> Option<String> {
    let text = text::read_text(&git_config_path(bucket_dir)?)?;
    let mut in_origin = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_origin = t.eq_ignore_ascii_case("[remote \"origin\"]");
            continue;
        }
        if in_origin {
            if let Some(rest) = t.strip_prefix("url") {
                if let Some(v) = rest.trim_start().strip_prefix('=') {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

pub fn count_json_recursive(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                count_json_recursive(&p)
            } else if p.extension().map(|x| x.eq_ignore_ascii_case("json")).unwrap_or(false) {
                1
            } else {
                0
            }
        })
        .sum()
}

pub fn scan(buckets_dir: &Path) -> Vec<BucketInfo> {
    let mut list: Vec<BucketInfo> = Vec::new();
    let Ok(entries) = std::fs::read_dir(buckets_dir) else {
        return list;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let source = git_remote(&dir);
        list.push(BucketInfo {
            name: entry.file_name().to_string_lossy().to_string(),
            is_git: source.is_some(),
            source,
            manifest_count: count_json_recursive(&dir.join("bucket")),
        });
    }
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_remote_from_config() {
        let tmp = tempfile::tempdir().unwrap();
        let bucket = tmp.path().join("main");
        std::fs::create_dir_all(bucket.join(".git")).unwrap();
        std::fs::write(
            bucket.join(".git").join("config"),
            "[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = https://gitee.com/scoop-installer/Main\n\tfetch = +refs/heads/*\n",
        )
        .unwrap();
        assert_eq!(
            git_remote(&bucket).as_deref(),
            Some("https://gitee.com/scoop-installer/Main")
        );
    }

    #[test]
    fn scans_buckets_with_counts_and_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir_all(main.join("bucket").join("sub")).unwrap();
        std::fs::create_dir_all(main.join(".git")).unwrap();
        std::fs::write(main.join(".git").join("config"), "[remote \"origin\"]\nurl = X\n").unwrap();
        std::fs::write(main.join("bucket").join("a.json"), "{}").unwrap();
        std::fs::write(main.join("bucket").join("sub").join("b.json"), "{}").unwrap();
        std::fs::write(main.join("bucket").join("readme.txt"), "x").unwrap();

        let extras = tmp.path().join("extras");
        std::fs::create_dir_all(extras.join("bucket")).unwrap();
        std::fs::write(extras.join("bucket").join("c.json"), "{}").unwrap();

        let list = scan(tmp.path());
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "extras");
        assert!(!list[0].is_git);
        assert_eq!(list[0].manifest_count, 1);
        assert_eq!(list[1].name, "main");
        assert!(list[1].is_git);
        assert_eq!(list[1].source.as_deref(), Some("X"));
        assert_eq!(list[1].manifest_count, 2);
    }

    #[test]
    fn missing_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(scan(&tmp.path().join("nope")).is_empty());
    }
}
