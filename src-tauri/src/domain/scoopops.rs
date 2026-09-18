use crate::error::AppError;

fn require(value: &str, kind: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return Err(AppError::Other(format!("{kind}不能为空且不能包含空白字符")));
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
    global: bool,
) -> Result<Vec<String>, AppError> {
    let name = require(name, "应用名")?;
    let bucket = optional(bucket, "桶名")?;
    let version = optional(version, "版本")?;
    let target = match (bucket, version) {
        (Some(b), Some(ver)) => format!("{b}/{name}@{ver}"),
        (Some(b), None) => format!("{b}/{name}"),
        (None, Some(ver)) => format!("{name}@{ver}"),
        (None, None) => name,
    };
    let mut args = vec!["install".to_string(), target];
    if global {
        args.push("-g".to_string());
    }
    Ok(args)
}

pub fn uninstall_args(name: &str, global: bool, purge: bool) -> Result<Vec<String>, AppError> {
    let name = require(name, "应用名")?;
    let mut args = vec!["uninstall".to_string(), name];
    if global {
        args.push("-g".to_string());
    }
    if purge {
        args.push("-p".to_string());
    }
    Ok(args)
}

pub fn update_args(name: Option<&str>, all: bool, global: bool) -> Result<Vec<String>, AppError> {
    let name = optional(name, "应用名")?;
    if all && name.is_some() {
        return Err(AppError::Other("不能同时指定 --all 与应用名".to_string()));
    }
    if all && global {
        return Err(AppError::Other("--all 与 --global 不能同时使用".to_string()));
    }
    if global && name.is_none() {
        return Err(AppError::Other("--global 需要指定应用名".to_string()));
    }
    if all {
        return Ok(vec!["update".to_string(), "-a".to_string()]);
    }
    let mut args = vec!["update".to_string()];
    if let Some(name) = name {
        args.push(name);
        if global {
            args.push("-g".to_string());
        }
    }
    Ok(args)
}

pub fn hold_args(name: &str, hold: bool, global: bool) -> Result<Vec<String>, AppError> {
    let name = require(name, "应用名")?;
    let mut args = vec![if hold { "hold" } else { "unhold" }.to_string(), name];
    if global {
        args.push("-g".to_string());
    }
    Ok(args)
}

pub fn bucket_add_args(name: &str, repo: Option<&str>) -> Result<Vec<String>, AppError> {
    let name = require(name, "桶名")?;
    let mut args = vec!["bucket".to_string(), "add".to_string(), name];
    if let Some(repo) = optional(repo, "仓库地址")? {
        args.push(repo);
    }
    Ok(args)
}

pub fn bucket_remove_args(name: &str) -> Result<Vec<String>, AppError> {
    let name = require(name, "桶名")?;
    Ok(vec!["bucket".to_string(), "rm".to_string(), name])
}

pub fn maintenance_args(op: &str) -> Result<Vec<String>, AppError> {
    let args: Vec<&str> = match op {
        "cleanup_all" => vec!["cleanup", "-a"],
        "cleanup_global" => vec!["cleanup", "-a", "-g"],
        "cache_rm_all" => vec!["cache", "rm", "-a"],
        "checkup" => vec!["checkup"],
        "reset_all" => vec!["reset", "-a"],
        other => return Err(AppError::Other(format!("未知的维护操作: {other}"))),
    };
    Ok(args.into_iter().map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn install_composes_bucket_version_and_flag() {
        assert_eq!(install_args("git", None, None, false).unwrap(), v(&["install", "git"]));
        assert_eq!(install_args("git", Some("main"), None, true).unwrap(), v(&["install", "main/git", "-g"]));
        assert_eq!(install_args("gh", None, Some("2.7.0"), false).unwrap(), v(&["install", "gh@2.7.0"]));
        assert_eq!(
            install_args("gh", Some("main"), Some("2.7.0"), false).unwrap(),
            v(&["install", "main/gh@2.7.0"])
        );
    }

    #[test]
    fn install_blank_optionals_ignored_blank_name_rejected() {
        assert_eq!(install_args("git", Some("  "), Some(""), false).unwrap(), v(&["install", "git"]));
        assert!(install_args("   ", None, None, false).is_err());
        assert!(install_args("git bash", None, None, false).is_err());
    }

    #[test]
    fn uninstall_flags() {
        assert_eq!(uninstall_args("git", false, false).unwrap(), v(&["uninstall", "git"]));
        assert_eq!(uninstall_args("git", true, true).unwrap(), v(&["uninstall", "git", "-g", "-p"]));
    }

    #[test]
    fn update_variants_and_validation() {
        assert_eq!(update_args(None, false, false).unwrap(), v(&["update"]));
        assert_eq!(update_args(Some("git"), false, false).unwrap(), v(&["update", "git"]));
        assert_eq!(update_args(Some("git"), false, true).unwrap(), v(&["update", "git", "-g"]));
        assert_eq!(update_args(None, true, false).unwrap(), v(&["update", "-a"]));
        assert!(update_args(Some("git"), true, false).is_err());
        assert!(update_args(None, false, true).is_err());
        assert!(update_args(None, true, true).is_err());
        assert_eq!(update_args(Some("   "), false, false).unwrap(), v(&["update"]));
        assert_eq!(update_args(Some("   "), true, false).unwrap(), v(&["update", "-a"]));
        assert!(update_args(Some("   "), false, true).is_err());
    }

    #[test]
    fn hold_and_unhold() {
        assert_eq!(hold_args("git", true, false).unwrap(), v(&["hold", "git"]));
        assert_eq!(hold_args("git", false, true).unwrap(), v(&["unhold", "git", "-g"]));
        assert!(hold_args("", true, false).is_err());
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
    fn maintenance_mapping_and_unknown() {
        assert_eq!(maintenance_args("cleanup_all").unwrap(), v(&["cleanup", "-a"]));
        assert_eq!(maintenance_args("cleanup_global").unwrap(), v(&["cleanup", "-a", "-g"]));
        assert_eq!(maintenance_args("cache_rm_all").unwrap(), v(&["cache", "rm", "-a"]));
        assert_eq!(maintenance_args("checkup").unwrap(), v(&["checkup"]));
        assert_eq!(maintenance_args("reset_all").unwrap(), v(&["reset", "-a"]));
        let err = maintenance_args("nuke").unwrap_err();
        assert!(err.to_string().contains("未知的维护操作"));
    }
}
