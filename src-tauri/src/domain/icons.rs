use std::io::Read;
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

pub(crate) fn base_content_type(ct: &str) -> String {
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

pub(crate) fn read_limited<R: Read>(mut reader: R, max: usize) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    reader.by_ref().take(max as u64 + 1).read_to_end(&mut buf).ok()?;
    if buf.len() > max {
        None
    } else {
        Some(buf)
    }
}

pub fn fetch_remote(url: &str) -> Option<(Vec<u8>, String)> {
    const MAX: usize = 512 * 1024;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("ScoopUI/0.1")
        .build()
        .ok()?;
    let resp = client.get(url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !is_image_content_type(&ct) {
        return None;
    }
    if resp.content_length().is_some_and(|len| len > MAX as u64) {
        return None;
    }
    let bytes = read_limited(resp, MAX)?;
    Some((bytes, ct))
}

pub(crate) fn resolve_with<F>(
    source: &str,
    cache_dir: &Path,
    proxy: Option<&str>,
    mut fetch: F,
) -> Option<String>
where
    F: FnMut(&str) -> Option<(Vec<u8>, String)>,
{
    let candidates = favicon_candidates(source);
    if candidates.is_empty() {
        return None;
    }
    if is_miss_fresh(cache_dir, source, Duration::from_secs(7 * 24 * 3600)) {
        return None;
    }
    if let Some((bytes, mime)) = load_icon(cache_dir, source) {
        return Some(to_data_url(&bytes, &mime));
    }
    for candidate in candidates {
        let effective = rewrite_with_proxy(&candidate, proxy);
        if let Some((bytes, mime)) = fetch(&effective) {
            let data_url = to_data_url(&bytes, &mime);
            let _ = save_icon(cache_dir, source, &bytes, &mime);
            return Some(data_url);
        }
    }
    let _ = mark_miss(cache_dir, source);
    None
}

pub fn resolve_icon(source: &str, cache_dir: &Path, proxy: Option<&str>) -> Option<String> {
    resolve_with(source, cache_dir, proxy, fetch_remote)
}

pub fn to_data_url(bytes: &[u8], mime: &str) -> String {
    use base64::Engine;
    format!(
        "data:{};base64,{}",
        base_content_type(mime),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
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

    #[test]
    fn resolve_uses_cache_before_network() {
        let tmp = tempfile::tempdir().unwrap();
        let source = "https://example.com";
        save_icon(tmp.path(), source, &[1, 2, 3], "image/png").unwrap();
        let out = resolve_with(source, tmp.path(), None, |_| {
            panic!("fetch must not be called when cache hits")
        });
        assert!(out.unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn resolve_negative_cache_blocks_fetch() {
        let tmp = tempfile::tempdir().unwrap();
        let source = "https://example.com";
        mark_miss(tmp.path(), source).unwrap();
        let out = resolve_with(source, tmp.path(), None, |_| {
            panic!("fetch must not be called while miss is fresh")
        });
        assert!(out.is_none());
    }

    #[test]
    fn resolve_fetch_failure_marks_miss_and_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let source = "https://example.com";
        let out = resolve_with(source, tmp.path(), None, |_| None);
        assert!(out.is_none());
        assert!(is_miss_fresh(tmp.path(), source, Duration::from_secs(7 * 24 * 3600)));
    }

    #[test]
    fn resolve_fetch_success_saves_and_reloads_from_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let source = "https://example.com";
        let first = resolve_with(source, tmp.path(), None, |url| {
            assert_eq!(url, "https://example.com/favicon.ico");
            Some((vec![0u8], "image/png".to_string()))
        })
        .unwrap();
        assert!(first.starts_with("data:image/png;base64,"));
        assert!(load_icon(tmp.path(), source).is_some());
        let second = resolve_with(source, tmp.path(), None, |_| {
            panic!("fetch must not be called after successful save")
        });
        assert_eq!(second.unwrap(), first);
    }

    #[test]
    fn to_data_url_encodes_base64() {
        assert_eq!(to_data_url(&[0u8], "image/png"), "data:image/png;base64,AA==");
    }

    #[test]
    fn read_limited_enforces_max_size() {
        assert_eq!(read_limited(&b"abc"[..], 3), Some(b"abc".to_vec()));
        assert_eq!(read_limited(&b"abcd"[..], 3), None);
        assert_eq!(read_limited(&b""[..], 3), Some(Vec::new()));
    }

    #[test]
    fn base_content_type_strips_parameters() {
        assert_eq!(base_content_type("image/png; charset=binary"), "image/png");
    }

    #[test]
    fn to_data_url_normalizes_mime_parameters() {
        assert_eq!(
            to_data_url(&[0u8], "image/png; charset=binary"),
            "data:image/png;base64,AA=="
        );
    }

    #[test]
    fn resolve_rewrites_github_candidate_through_proxy() {
        let tmp = tempfile::tempdir().unwrap();
        let source = "https://github.com/kornelski/pngquant";
        let out = resolve_with(source, tmp.path(), Some("https://proxy.test"), |url| {
            assert_eq!(url, "https://proxy.test/https://github.com/kornelski.png");
            Some((vec![0u8], "image/png".to_string()))
        });
        assert!(out.unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn resolve_returns_image_when_cache_save_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let blocked = tmp.path().join("blocked");
        std::fs::write(&blocked, b"occupied").unwrap();
        let source = "https://example.com";
        let out = resolve_with(source, &blocked, None, |url| {
            assert_eq!(url, "https://example.com/favicon.ico");
            Some((vec![0u8], "image/png".to_string()))
        });
        assert_eq!(out.unwrap(), "data:image/png;base64,AA==");
    }
}
