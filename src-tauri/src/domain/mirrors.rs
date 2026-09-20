use std::time::Duration;

use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MirrorPresets {
    pub official: String,
    pub gitee: String,
    pub proxy: String,
}

const GITHUB_PREFIX: &str = "https://github.com/scoopinstaller/";
const GITEE_PREFIX: &str = "https://gitee.com/scoop-installer/";

fn strip_proxy_prefix(remote: &str, proxy: &str) -> String {
    let prefix = format!("{}/", proxy.trim_end_matches('/'));
    remote.strip_prefix(&prefix).unwrap_or(remote).to_string()
}

pub fn presets(remote: &str, proxy: &str) -> Option<MirrorPresets> {
    let base = strip_proxy_prefix(remote.trim(), proxy);
    let lower = base.to_ascii_lowercase();
    let tail = if lower.starts_with(GITHUB_PREFIX) {
        &base[GITHUB_PREFIX.len()..]
    } else if lower.starts_with(GITEE_PREFIX) {
        &base[GITEE_PREFIX.len()..]
    } else {
        return None;
    };
    let name = tail.trim_end_matches('/').trim_end_matches(".git");
    if name.is_empty() || name.contains('/') || name.contains(' ') {
        return None;
    }
    let base_url = base.trim_end_matches('/').trim_end_matches(".git");
    let proxy_base = proxy.trim_end_matches('/');
    Some(MirrorPresets {
        official: format!("https://github.com/ScoopInstaller/{name}"),
        gitee: format!("https://gitee.com/scoop-installer/{name}"),
        proxy: format!("{proxy_base}/{base_url}"),
    })
}

pub fn apply_source(remote: &str, source: &str, proxy: &str) -> Result<String, AppError> {
    let p = presets(remote, proxy).ok_or_else(|| {
        AppError::Other("该仓库不是官方 GitHub/gitee 已知仓库，无法一键切换源".to_string())
    })?;
    match source {
        "official" => Ok(p.official),
        "gitee" => Ok(p.gitee),
        "proxy" => Ok(p.proxy),
        other => Err(AppError::Other(format!("未知的镜像源选项: {other}"))),
    }
}

pub fn run_git(dir: &str, args: &[&str]) -> Result<(), AppError> {
    let output = std::process::Command::new("git")
        .args(["-C", dir])
        .args(args)
        .output()
        .map_err(|e| AppError::Other(format!("运行 git 失败: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Other(format!("git 执行失败: {}", stderr.trim())));
    }
    Ok(())
}

pub fn http_get_text(url: &str) -> Option<(u16, String)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("ScoopUI/0.1")
        .build()
        .ok()?;
    let resp = client.get(url).send().ok()?;
    let status = resp.status().as_u16();
    let body = resp.text().unwrap_or_default();
    let snippet: String = body.chars().take(200).collect();
    Some((status, snippet))
}

pub fn classify_cn(body: &str) -> bool {
    body.contains("中国") || body.contains("内网")
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "https://scoop.201704.xyz";

    #[test]
    fn derives_presets_from_gitee_and_github_remotes() {
        let from_gitee = presets("https://gitee.com/scoop-installer/Main", P).unwrap();
        assert_eq!(from_gitee.official, "https://github.com/ScoopInstaller/Main");
        assert_eq!(from_gitee.gitee, "https://gitee.com/scoop-installer/Main");
        assert_eq!(from_gitee.proxy, "https://scoop.201704.xyz/https://gitee.com/scoop-installer/Main");

        let proxied = presets("https://scoop.201704.xyz/https://gitee.com/scoop-installer/Extras", P).unwrap();
        assert_eq!(proxied.official, "https://github.com/ScoopInstaller/Extras");
        assert_eq!(proxied.proxy, "https://scoop.201704.xyz/https://gitee.com/scoop-installer/Extras");

        let with_git = presets("https://github.com/ScoopInstaller/Scoop.git", P).unwrap();
        assert_eq!(with_git.official, "https://github.com/ScoopInstaller/Scoop");
    }

    #[test]
    fn rejects_unknown_hosts() {
        assert!(presets("https://gitlab.com/foo/bar", P).is_none());
        assert!(presets("not a url", P).is_none());
        assert!(presets("https://github.com/Other/Repo", P).is_none());
    }

    #[test]
    fn apply_source_validates() {
        let remote = "https://gitee.com/scoop-installer/Main";
        assert_eq!(apply_source(remote, "official", P).unwrap(), "https://github.com/ScoopInstaller/Main");
        assert_eq!(
            apply_source(remote, "proxy", P).unwrap(),
            "https://scoop.201704.xyz/https://gitee.com/scoop-installer/Main"
        );
        assert!(apply_source(remote, "nope", P).is_err());
        assert!(apply_source("https://gitlab.com/foo/bar", "official", P).is_err());
    }

    #[test]
    fn classify_cn_detects_china() {
        assert!(classify_cn("中国 广东 深圳 电信"));
        assert!(classify_cn("内网 10.0.0.1"));
        assert!(!classify_cn("United States"));
    }

    #[test]
    fn run_git_roundtrip_in_temp_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().display().to_string();
        run_git(&dir, &["init", "-q"]).unwrap();
        run_git(&dir, &["remote", "add", "origin", "https://gitee.com/scoop-installer/Main"]).unwrap();
        run_git(&dir, &["remote", "set-url", "origin", "https://github.com/ScoopInstaller/Main"]).unwrap();
        let out = std::process::Command::new("git")
            .args(["-C", &dir, "remote", "get-url", "origin"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "https://github.com/ScoopInstaller/Main");
        assert!(run_git(&tmp.path().join("nope").display().to_string(), &["status"]).is_err());
    }
}
