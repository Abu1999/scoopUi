use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use url::Url;

use crate::AppError;

const IMAGE_EXTS: [&str; 6] = ["ico", "png", "jpg", "jpeg", "svg", "webp"];

const GITHUB_HOSTS: [&str; 4] = [
    "github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
    "githubusercontent.com",
];

fn has_image_ext(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    IMAGE_EXTS.iter().any(|ext| lower.ends_with(&format!(".{ext}")))
}

pub fn favicon_candidates(source: &str) -> Vec<String> {
    if has_image_ext(source) {
        return vec![source.to_string()];
    }
    let Ok(url) = Url::parse(source) else {
        return Vec::new();
    };
    let Some(host) = url.host_str() else {
        return Vec::new();
    };
    if host == "github.com" {
        if let Some(owner) = url.path_segments().and_then(|mut segments| segments.next()) {
            if !owner.is_empty() {
                return vec![format!("https://github.com/{owner}.png")];
            }
        }
    }
    vec![format!(
        "{}://{}/favicon.ico",
        url.scheme(),
        &url[url::Position::BeforeHost..url::Position::AfterPort]
    )]
}

pub fn rewrite_with_proxy(url: &str, proxy: Option<&str>) -> String {
    let Some(proxy) = proxy.map(str::trim).filter(|p| !p.is_empty()) else {
        return url.to_string();
    };
    let Ok(parsed) = Url::parse(url) else {
        return url.to_string();
    };
    match parsed.host_str() {
        Some(host) if GITHUB_HOSTS.contains(&host) => {
            format!("{}/{url}", proxy.trim_end_matches('/'))
        }
        _ => url.to_string(),
    }
}

fn base_content_type(ct: &str) -> String {
    ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase()
}

pub fn is_image_content_type(ct: &str) -> bool {
    base_content_type(ct).starts_with("image/")
}

pub fn ext_from_content_type(ct: &str) -> &'static str {
    match base_content_type(ct).as_str() {
        "image/x-icon" | "image/vnd.microsoft.icon" | "image/icon" | "image/ico" => "ico",
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/svg+xml" | "image/svg" => "svg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "img",
    }
}

pub fn cache_key(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn save_icon(dir: &Path, url: &str, bytes: &[u8], mime: &str) -> Result<(), AppError> {
    std::fs::create_dir_all(dir)?;
    let key = cache_key(url);
    std::fs::write(dir.join(format!("{key}.bin")), bytes)?;
    let meta = serde_json::json!({ "mime": mime, "url": url }).to_string();
    std::fs::write(dir.join(format!("{key}.json")), meta)?;
    Ok(())
}

pub fn load_icon(dir: &Path, url: &str) -> Option<(Vec<u8>, String)> {
    let key = cache_key(url);
    let bytes = std::fs::read(dir.join(format!("{key}.bin"))).ok()?;
    let meta = std::fs::read_to_string(dir.join(format!("{key}.json"))).ok()?;
    let value: serde_json::Value = serde_json::from_str(&meta).ok()?;
    let mime = value.get("mime")?.as_str()?.to_string();
    Some((bytes, mime))
}

pub fn mark_miss(dir: &Path, url: &str) -> Result<(), AppError> {
    std::fs::create_dir_all(dir)?;
    let key = cache_key(url);
    std::fs::write(dir.join(format!("{key}.miss")), now_secs().to_string())?;
    Ok(())
}

pub fn is_miss_fresh(dir: &Path, url: &str, max_age: Duration) -> bool {
    let key = cache_key(url);
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{key}.miss"))) else {
        return false;
    };
    let Ok(stamp) = text.trim().parse::<u64>() else {
        return false;
    };
    now_secs().saturating_sub(stamp) < max_age.as_secs()
}

pub fn default_proxy() -> &'static str {
    "https://scoop.201704.xyz"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn candidates_for_homepages_and_images() {
        assert_eq!(
            favicon_candidates("https://gitforwindows.org"),
            vec!["https://gitforwindows.org/favicon.ico"]
        );
        assert_eq!(
            favicon_candidates("https://cmake.org/download/"),
            vec!["https://cmake.org/favicon.ico"]
        );
        assert_eq!(
            favicon_candidates("https://example.com:8443/a/b?q=1"),
            vec!["https://example.com:8443/favicon.ico"]
        );
        assert_eq!(
            favicon_candidates("https://github.com/flutter/flutter"),
            vec!["https://github.com/flutter.png"]
        );
        assert_eq!(
            favicon_candidates("https://raw.githubusercontent.com/a/b/c.png"),
            vec!["https://raw.githubusercontent.com/a/b/c.png"]
        );
        assert!(favicon_candidates("not a url").is_empty());
    }

    #[test]
    fn proxy_rewrite_only_for_github_hosts() {
        let p = Some("https://scoop.201704.xyz");
        assert_eq!(
            rewrite_with_proxy("https://github.com/flutter.png", p),
            "https://scoop.201704.xyz/https://github.com/flutter.png"
        );
        assert_eq!(
            rewrite_with_proxy("https://gitforwindows.org/favicon.ico", p),
            "https://gitforwindows.org/favicon.ico"
        );
        assert_eq!(
            rewrite_with_proxy("https://github.com/x.png", None),
            "https://github.com/x.png"
        );
        assert_eq!(
            rewrite_with_proxy(
                "https://raw.githubusercontent.com/a/b.ico",
                Some("https://scoop.201704.xyz/")
            ),
            "https://scoop.201704.xyz/https://raw.githubusercontent.com/a/b.ico"
        );
    }

    #[test]
    fn content_type_checks() {
        assert!(is_image_content_type("image/x-icon"));
        assert!(is_image_content_type("image/png; charset=binary"));
        assert!(!is_image_content_type("text/html"));
        assert_eq!(ext_from_content_type("image/vnd.microsoft.icon"), "ico");
        assert_eq!(ext_from_content_type("image/png"), "png");
        assert_eq!(ext_from_content_type("application/octet-stream"), "img");
    }

    #[test]
    fn cache_roundtrip_and_miss() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://x.test/favicon.ico";
        assert!(load_icon(tmp.path(), url).is_none());
        save_icon(tmp.path(), url, &[1, 2, 3], "image/png").unwrap();
        let (bytes, mime) = load_icon(tmp.path(), url).unwrap();
        assert_eq!(bytes, vec![1, 2, 3]);
        assert_eq!(mime, "image/png");

        mark_miss(tmp.path(), url).unwrap();
        assert!(is_miss_fresh(tmp.path(), url, Duration::from_secs(7 * 24 * 3600)));
        assert!(!is_miss_fresh(tmp.path(), url, Duration::ZERO));
    }

    #[test]
    fn cache_key_is_stable_and_hex() {
        let k1 = cache_key("https://a/b");
        assert_eq!(k1, cache_key("https://a/b"));
        assert_ne!(k1, cache_key("https://a/c"));
        assert!(k1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
