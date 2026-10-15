use serde::Serialize;
use serde_json::{json, Value};
use tauri::State;

use crate::commands::{context, roots::resolve_scoop_dir};
use crate::domain::buckets;
use crate::domain::configfile;
use crate::domain::icons;
use crate::domain::mirrors::{self, MirrorPresets};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct BucketMirrorInfo {
    pub name: String,
    pub remote: Option<String>,
    pub presets: Option<MirrorPresets>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MirrorInfo {
    pub scoop_repo: Option<String>,
    pub scoop_remote: Option<String>,
    pub presets: Option<MirrorPresets>,
    pub url_proxy: Option<String>,
    pub default_url_proxy: String,
    pub proxy: Option<String>,
    pub buckets: Vec<BucketMirrorInfo>,
    pub aria2_installed: bool,
    pub aria2_enabled: bool,
    pub aria2_split: Option<u64>,
    pub aria2_max_connection: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyTest {
    pub exit_info: Option<String>,
    pub in_china: Option<bool>,
    pub proxy_reachable: bool,
    pub proxy_info: Option<String>,
}

fn config_str(values: &Value, key: &str) -> Option<String> {
    values
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn resolved_config_path(
    ctx: &crate::commands::Context,
    state: &AppState,
) -> Result<(std::path::PathBuf, std::path::PathBuf), AppError> {
    let root = resolve_scoop_dir(ctx, state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root);
    Ok((root, config_path))
}

#[tauri::command(async)]
pub fn get_mirror_info(state: State<'_, AppState>) -> Result<MirrorInfo, AppError> {
    let ctx = context();
    let (root, config_path) = resolved_config_path(&ctx, &state)?;
    let values = configfile::load(&config_path);
    let default_proxy = icons::default_proxy().to_string();
    let scoop_repo = config_str(&values, "scoop_repo");
    let scoop_dir = root.join("apps").join("scoop").join("current");
    let scoop_remote = buckets::git_remote(&scoop_dir);
    let scoop_presets = scoop_remote
        .as_deref()
        .or(scoop_repo.as_deref())
        .and_then(|r| mirrors::presets(r, &default_proxy));
    let buckets_dir = root.join("buckets");
    let mut bucket_list: Vec<BucketMirrorInfo> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&buckets_dir) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let remote = buckets::git_remote(&dir);
            bucket_list.push(BucketMirrorInfo {
                name: entry.file_name().to_string_lossy().to_string(),
                presets: remote
                    .as_deref()
                    .and_then(|r| mirrors::presets(r, &default_proxy)),
                remote,
            });
        }
    }
    bucket_list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(MirrorInfo {
        scoop_repo,
        scoop_remote,
        presets: scoop_presets,
        url_proxy: config_str(&values, "url_proxy"),
        default_url_proxy: default_proxy,
        proxy: config_str(&values, "proxy"),
        buckets: bucket_list,
        aria2_installed: root.join("apps").join("aria2").join("current").is_dir(),
        aria2_enabled: values
            .get("aria2-enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        aria2_split: values.get("aria2-split").and_then(|v| v.as_u64()),
        aria2_max_connection: values.get("aria2-max-connection-per-server").and_then(|v| v.as_u64()),
    })
}

fn validate_bucket_name(bucket: &str) -> Result<(), AppError> {
    let normalized = bucket.trim_end_matches(|c| c == ' ' || c == '.');
    let invalid = bucket.is_empty()
        || normalized.is_empty()
        || normalized != bucket
        || bucket
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control());
    if invalid {
        return Err(AppError::Other(format!("软件源名称非法: {bucket}")));
    }
    Ok(())
}

#[tauri::command]
pub async fn set_bucket_source(
    state: State<'_, AppState>,
    bucket: String,
    source: String,
) -> Result<String, AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    validate_bucket_name(&bucket)?;
    let dir = root.join("buckets").join(&bucket);
    if !dir.is_dir() {
        return Err(AppError::Other(format!("软件源不存在: {bucket}")));
    }
    let remote = buckets::git_remote(&dir)
        .ok_or_else(|| AppError::Other(format!("软件源不是 git 仓库，无法切换源: {bucket}")))?;
    let url = mirrors::apply_source(&remote, &source, icons::default_proxy())?;
    let dir_text = dir.display().to_string();
    let shims = root.join("shims");
    let applied = url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        mirrors::run_git(&dir_text, &["remote", "set-url", "origin", &applied], &shims)
    })
    .await
    .map_err(|e| AppError::Other(format!("切换源失败: {e}")))??;
    Ok(url)
}

#[tauri::command]
pub async fn set_scoop_source(
    state: State<'_, AppState>,
    source: String,
) -> Result<String, AppError> {
    let ctx = context();
    let (root, config_path) = resolved_config_path(&ctx, &state)?;
    let values = configfile::load(&config_path);
    let dir = root.join("apps").join("scoop").join("current");
    let current = buckets::git_remote(&dir)
        .or_else(|| config_str(&values, "scoop_repo"))
        .ok_or_else(|| AppError::Other("无法读取 scoop 仓库地址".to_string()))?;
    let url = mirrors::apply_source(&current, &source, icons::default_proxy())?;
    let dir_text = dir.display().to_string();
    let shims = root.join("shims");
    let applied = url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        mirrors::run_git(&dir_text, &["remote", "set-url", "origin", &applied], &shims)
    })
    .await
    .map_err(|e| AppError::Other(format!("切换源失败: {e}")))??;
    configfile::set_key(&config_path, "scoop_repo", Some(Value::String(url.clone())))?;
    Ok(url)
}

fn validate_proxy_url(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim().trim_end_matches('/');
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err(AppError::Other("代理地址必须以 http:// 或 https:// 开头".to_string()));
    }
    Ok(trimmed.to_string())
}

#[tauri::command(async)]
pub fn set_url_proxy(state: State<'_, AppState>, value: Option<String>) -> Result<(), AppError> {
    let ctx = context();
    let (_root, config_path) = resolved_config_path(&ctx, &state)?;
    let stored = match value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        Some(v) => Some(Value::String(validate_proxy_url(&v)?)),
        None => None,
    };
    configfile::set_key(&config_path, "url_proxy", stored)
}

#[tauri::command(async)]
pub fn set_proxy(state: State<'_, AppState>, value: Option<String>) -> Result<(), AppError> {
    let ctx = context();
    let (_root, config_path) = resolved_config_path(&ctx, &state)?;
    let stored = match value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        Some(v) => {
            if v.chars().any(char::is_whitespace) || !v.contains(':') {
                return Err(AppError::Other("代理格式应为 host:port".to_string()));
            }
            Some(Value::String(v))
        }
        None => None,
    };
    configfile::set_key(&config_path, "proxy", stored)
}

fn validate_range(name: &str, value: Option<u32>) -> Result<(), AppError> {
    if let Some(v) = value {
        if v == 0 || v > 64 {
            return Err(AppError::Other(format!("{name} 取值范围为 1-64")));
        }
    }
    Ok(())
}

#[tauri::command(async)]
pub fn set_aria2(
    state: State<'_, AppState>,
    enabled: bool,
    split: Option<u32>,
    max_connection: Option<u32>,
) -> Result<(), AppError> {
    validate_range("aria2-split", split)?;
    validate_range("aria2-max-connection-per-server", max_connection)?;
    let ctx = context();
    let (_root, config_path) = resolved_config_path(&ctx, &state)?;
    configfile::set_key(&config_path, "aria2-enabled", Some(json!(enabled)))?;
    configfile::set_key(
        &config_path,
        "aria2-split",
        split.map(|v| json!(v)),
    )?;
    configfile::set_key(
        &config_path,
        "aria2-max-connection-per-server",
        max_connection.map(|v| json!(v)),
    )?;
    Ok(())
}

#[tauri::command]
pub async fn test_url_proxy(
    state: State<'_, AppState>,
    proxy: Option<String>,
) -> Result<ProxyTest, AppError> {
    let ctx = context();
    let (_root, config_path) = resolved_config_path(&ctx, &state)?;
    let values = configfile::load(&config_path);
    let effective = proxy
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| config_str(&values, "url_proxy"))
        .unwrap_or_else(|| icons::default_proxy().to_string());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let direct = mirrors::http_get_text("https://ip.glimmer.ltd/0");
        let (exit_info, in_china) = match direct {
            Some((_status, body)) if !body.trim().is_empty() => {
                (Some(body.clone()), Some(mirrors::classify_cn(&body)))
            }
            _ => (None, None),
        };
        let probe_url = format!("{effective}/https://ip.glimmer.ltd/0");
        let (proxy_reachable, proxy_info) = match mirrors::http_get_text(&probe_url) {
            Some((status, body)) => (status == 200 && !body.trim().is_empty(), Some(body)),
            None => (false, None),
        };
        ProxyTest {
            exit_info,
            in_china,
            proxy_reachable,
            proxy_info,
        }
    })
    .await
    .map_err(|e| AppError::Other(format!("连通测试失败: {e}")))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_bucket_name_rejects_traversal_and_separators() {
        assert!(validate_bucket_name("main").is_ok());
        assert!(validate_bucket_name("my-bucket_2").is_ok());
        for bad in [
            "", ".", "..", ".. ", ".. .", "main.", "main ", " ", "../main", "a/b", "a\\b",
            "C:\\scoop", "a:b", "a*b", "a\nb",
        ] {
            assert!(validate_bucket_name(bad).is_err(), "{bad:?} 应被拒绝");
        }
        let err = validate_bucket_name("a/b").unwrap_err().to_string();
        assert!(err.contains("软件源名称非法"), "实际: {err}");
    }
}
