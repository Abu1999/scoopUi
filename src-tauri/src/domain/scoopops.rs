use crate::error::AppError;

fn require(value: &str, kind: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return Err(AppError::Other(format!("{kind}不能为空且不能包含空白字符")));
    }
    if trimmed.starts_with('-') {
        return Err(AppError::Other(format!("{kind}不能以 - 开头")));
    }
    Ok(trimmed.to_string())
}

fn optional(value: Option<&str>, kind: &str) -> Result<Option<String>, AppError> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => require(v, kind).map(Some),
        None => Ok(None),
    }
}

pub fn install_args(
    name: &str,
    bucket: Option<&str>,
    version: Option<&str>,
) -> Result<Vec<String>, AppError> {
    let name = require(name, "应用名")?;
    let bucket = optional(bucket, "软件源名称")?;
    let version = optional(version, "版本")?;
    let target = match (bucket, version) {
        (Some(b), Some(ver)) => format!("{b}/{name}@{ver}"),
        (Some(b), None) => format!("{b}/{name}"),
        (None, Some(ver)) => format!("{name}@{ver}"),
        (None, None) => name,
    };
    Ok(vec!["install".to_string(), target])
}

pub fn uninstall_args(name: &str, purge: bool) -> Result<Vec<String>, AppError> {
    let name = require(name, "应用名")?;
    let mut args = vec!["uninstall".to_string(), name];
    if purge {
        args.push("-p".to_string());
    }
    Ok(args)
}

pub fn update_args(name: Option<&str>, all: bool) -> Result<Vec<String>, AppError> {
    let name = optional(name, "应用名")?;
    if all && name.is_some() {
        return Err(AppError::Other("不能同时指定 --all 与应用名".to_string()));
    }
    if all {
        return Ok(vec!["update".to_string(), "-a".to_string()]);
    }
    let mut args = vec!["update".to_string()];
    if let Some(name) = name {
        args.push(name);
    }
    Ok(args)
}

pub fn hold_args(name: &str, hold: bool) -> Result<Vec<String>, AppError> {
    let name = require(name, "应用名")?;
    Ok(vec![if hold { "hold" } else { "unhold" }.to_string(), name])
}

pub fn bucket_add_args(name: &str, repo: Option<&str>) -> Result<Vec<String>, AppError> {
    let name = require(name, "软件源名称")?;
    let mut args = vec!["bucket".to_string(), "add".to_string(), name];
    if let Some(repo) = optional(repo, "仓库地址")? {
        args.push(repo);
    }
    Ok(args)
}

pub fn bucket_remove_args(name: &str) -> Result<Vec<String>, AppError> {
    let name = require(name, "软件源名称")?;
    Ok(vec!["bucket".to_string(), "rm".to_string(), name])
}

pub fn maintenance_args(op: &str) -> Result<Vec<String>, AppError> {
    let args: Vec<&str> = match op {
        "cleanup_all" => vec!["cleanup", "-a"],
        "cache_rm_all" => vec!["cache", "rm", "-a"],
        "checkup" => vec!["checkup"],
        "reset_all" => vec!["reset", "-a"],
        other => return Err(AppError::Other(format!("未知的维护操作: {other}"))),
    };
    Ok(args.into_iter().map(str::to_string).collect())
}

pub fn parse_known_buckets(output: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in output.lines() {
        let name = line.trim().to_ascii_lowercase();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn install_composes_bucket_and_version() {
        assert_eq!(install_args("git", None, None).unwrap(), v(&["install", "git"]));
        assert_eq!(install_args("git", Some("main"), None).unwrap(), v(&["install", "main/git"]));
        assert_eq!(install_args("gh", None, Some("2.7.0")).unwrap(), v(&["install", "gh@2.7.0"]));
        assert_eq!(
            install_args("gh", Some("main"), Some("2.7.0")).unwrap(),
            v(&["install", "main/gh@2.7.0"])
        );
    }

    #[test]
    fn install_args_never_adds_global_flag() {
        assert_eq!(install_args("git", None, None).unwrap(), vec!["install", "git"]);
    }

    #[test]
    fn install_blank_optionals_ignored_blank_name_rejected() {
        assert_eq!(install_args("git", Some("  "), Some("")).unwrap(), v(&["install", "git"]));
        assert!(install_args("   ", None, None).is_err());
        assert!(install_args("git bash", None, None).is_err());
    }

    #[test]
    fn uninstall_flags() {
        assert_eq!(uninstall_args("git", false).unwrap(), v(&["uninstall", "git"]));
        assert_eq!(uninstall_args("git", true).unwrap(), v(&["uninstall", "git", "-p"]));
    }

    #[test]
    fn update_variants_and_validation() {
        assert_eq!(update_args(None, false).unwrap(), v(&["update"]));
        assert_eq!(update_args(Some("git"), false).unwrap(), v(&["update", "git"]));
        assert_eq!(update_args(None, true).unwrap(), v(&["update", "-a"]));
        assert!(update_args(Some("git"), true).is_err());
        assert_eq!(update_args(Some("   "), false).unwrap(), v(&["update"]));
        assert_eq!(update_args(Some("   "), true).unwrap(), v(&["update", "-a"]));
    }

    #[test]
    fn hold_and_unhold() {
        assert_eq!(hold_args("git", true).unwrap(), v(&["hold", "git"]));
        assert_eq!(hold_args("git", false).unwrap(), v(&["unhold", "git"]));
        assert!(hold_args("", true).is_err());
    }

    #[test]
    fn bucket_add_and_remove() {
        assert_eq!(bucket_add_args("java", None).unwrap(), v(&["bucket", "add", "java"]));
        assert_eq!(
            bucket_add_args("extras", Some("https://github.com/ScoopInstaller/Extras.git")).unwrap(),
            v(&["bucket", "add", "extras", "https://github.com/ScoopInstaller/Extras.git"])
        );
        assert_eq!(bucket_remove_args("java").unwrap(), v(&["bucket", "rm", "java"]));
        assert!(bucket_add_args("", None).is_err());
        assert!(bucket_remove_args("a b").is_err());
    }

    #[test]
    fn bucket_errors_use_software_source_wording() {
        let err = bucket_add_args("", None).unwrap_err().to_string();
        assert!(err.contains("软件源名称"), "实际: {err}");
        let err = bucket_remove_args("a b").unwrap_err().to_string();
        assert!(err.contains("软件源名称"), "实际: {err}");
    }

    #[test]
    fn rejects_values_starting_with_dash() {
        assert!(install_args("-g", None, None).is_err());
        assert!(install_args("git", Some("--all"), None).is_err());
        assert!(install_args("git", None, Some("-x")).is_err());
        assert!(update_args(Some("-a"), false).is_err());
        assert!(hold_args("--global", true).is_err());
        assert!(bucket_add_args("-x", None).is_err());
        assert!(bucket_remove_args("-x").is_err());
        assert!(uninstall_args("-p", false).is_err());
    }

    #[test]
    fn maintenance_mapping_and_unknown() {
        assert_eq!(maintenance_args("cleanup_all").unwrap(), v(&["cleanup", "-a"]));
        assert!(maintenance_args("cleanup_global").is_err());
        assert_eq!(maintenance_args("cache_rm_all").unwrap(), v(&["cache", "rm", "-a"]));
        assert_eq!(maintenance_args("checkup").unwrap(), v(&["checkup"]));
        assert_eq!(maintenance_args("reset_all").unwrap(), v(&["reset", "-a"]));
        let err = maintenance_args("nuke").unwrap_err();
        assert!(err.to_string().contains("未知的维护操作"));
    }

    #[test]
    fn parses_known_buckets_filtering_noise() {
        let out = "main\n extras \n\nJAVA\nmain\nWarning: something\nphp\nbad name\n";
        assert_eq!(parse_known_buckets(out), vec!["main", "extras", "java", "php"]);
    }
}
