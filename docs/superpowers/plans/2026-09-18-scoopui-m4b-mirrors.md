# ScoopUI M4b（镜像源 / URL_PROXY / aria2）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 设置页提供三块能力：①镜像源切换（scoop 自身仓库与各桶的官方/gitee/201704 代理，写入 git remote + `scoop_repo` 配置）；②URL_PROXY 开关（读写 `url_proxy`、连通测试与出口 IP（是否在中国）判定）与 `proxy` 设置；③aria2 一键安装与参数（`aria2-enabled`/`aria2-split`/`aria2-max-connection-per-server`）。

**Architecture:** `configfile::set_key`（原子写 + 损坏备份）→ `domain/mirrors.rs`（预设推导 / git 调用 / HTTP 文本探针，纯逻辑可测）→ `commands/mirrors.rs`（7 个命令）→ 前端三个设置区块组件 + SettingsPage 组合。远端 URL 推导基于当前 remote 反解仓库名（剥离代理前缀后匹配 `github.com/ScoopInstaller/*` 或 `gitee.com/scoop-installer/*`），非官方/非 gitee 桶不提供一键预设。

**Tech Stack:** Rust（Tauri 2，复用 reqwest 阻塞客户端）、Vue 3 + Naive UI + Pinia + Vitest。无新增依赖。

## Global Constraints

- 契约（前端依赖，字面一致）：
  - `get_mirror_info() -> MirrorInfo`；`set_bucket_source(bucket, source) -> String`（返回应用后的 URL）；`set_scoop_source(source) -> String`；`set_url_proxy(value: string|null)`；`set_proxy(value: string|null)`；`set_aria2(enabled: bool, split: number|null, max_connection: number|null)`；`test_url_proxy(proxy: string|null) -> ProxyTest`
  - `MirrorInfo { scoop_repo, scoop_remote, presets, url_proxy, default_url_proxy, proxy, buckets: [{name, remote, presets}], aria2_installed, aria2_enabled, aria2_split, aria2_max_connection }`（snake_case）
  - `ProxyTest { exit_info, in_china, proxy_reachable, proxy_info }`
  - `source ∈ {official, gitee, proxy}`；错误中文；`url_proxy` 空/非法 → 中文错误；`split/max_connection` 范围 1..=64
- URL_PROXY 机制（本机 fork `core.ps1 Url_Proxy` 实测）：`url_proxy` 未设置时默认 `https://scoop.201704.xyz`；探针 `https://ip.glimmer.ltd/0` 返回含“中国”即在 CN；含“内网”视为直连；失败一律走代理
- aria2 键与默认：`aria2-enabled`（bool，scoop 默认 true）、`aria2-split`（默认 5）、`aria2-max-connection-per-server`（默认 5）；key 一律小写
- config 写入：先 `configfile::resolve`（portable 优先）再原子写（tmp+rename）；损坏文件先备份 `.json.bak`
- 阻塞 IO（git/HTTP）必须 `tauri::async_runtime::spawn_blocking`；前端操作在 `logs.busy` 时禁用
- UI 文案全中文；代码不加注释；不加依赖
- 验证门：`cargo test` 全绿、`pnpm test` 全绿、`pnpm build` 无错
- 初始 BASE 提交：`a376964`

---

### Task 1: configfile 写入 + mirrors 领域（预设/探针/git）

**Files:**
- Modify: `src-tauri/src/domain/configfile.rs`（`set_key`）
- Create: `src-tauri/src/domain/mirrors.rs`
- Modify: `src-tauri/src/domain/mod.rs`
- Test: `src-tauri/tests/proxy_probe.rs`（`#[ignore]` 真实网络）

**Interfaces:**
- Produces:
  - `configfile::set_key(path: &Path, key: &str, value: Option<serde_json::Value>) -> Result<(), AppError>`
  - `mirrors::DEFAULT_PROXY: &str` via `crate::domain::icons::default_proxy()`
  - `mirrors::presets(remote: &str, proxy: &str) -> Option<MirrorPresets>`；`MirrorPresets { official, gitee, proxy }`
  - `mirrors::apply_source(remote: &str, source: &str, proxy: &str) -> Result<String, AppError>`
  - `mirrors::run_git(dir: &str, args: &[&str]) -> Result<(), AppError>`
  - `mirrors::http_get_text(url: &str) -> Option<(u16, String)>`
  - `mirrors::classify_cn(body: &str) -> bool`

- [ ] **Step 1: 写失败测试**

`configfile.rs` 测试追加：

```rust
    #[test]
    fn set_key_preserves_other_keys_and_removes_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"scoop_branch":"master","Keep":1}"#).unwrap();
        set_key(&path, "URL_PROXY", Some(serde_json::json!("https://x"))).unwrap();
        let v = load(&path);
        assert_eq!(v.get("url_proxy").and_then(|x| x.as_str()), Some("https://x"));
        assert_eq!(v.get("scoop_branch").and_then(|x| x.as_str()), Some("master"));
        assert_eq!(v.get("keep").and_then(|x| x.as_i64()), Some(1));
        set_key(&path, "url_proxy", None).unwrap();
        assert!(load(&path).get("url_proxy").is_none());
    }

    #[test]
    fn set_key_backs_up_corrupt_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, "{oops").unwrap();
        set_key(&path, "proxy", Some(serde_json::json!("127.0.0.1:7890"))).unwrap();
        assert!(tmp.path().join("config.json.bak").is_file());
        assert_eq!(load(&path).get("proxy").and_then(|x| x.as_str()), Some("127.0.0.1:7890"));
    }
```

创建 `mirrors.rs`（先只放测试）：

```rust
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
            "https://scoop.201704.xyz/https://github.com/ScoopInstaller/Main"
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
```

创建 `src-tauri/tests/proxy_probe.rs`：

```rust
use scoopui_lib::domain::mirrors;

#[test]
#[ignore = "需要真实网络；用 cargo test --test proxy_probe -- --ignored 运行"]
fn probes_ip_service() {
    let (status, body) = mirrors::http_get_text("https://ip.glimmer.ltd/0").expect("应能访问");
    assert!(status == 200, "状态码 {status}");
    assert!(!body.trim().is_empty());
}
```

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test configfile:: mirrors:: --lib`（分开跑）→ 红。

- [ ] **Step 2: 实现**

`configfile.rs` 追加：

```rust
use crate::error::AppError;

pub fn set_key(path: &Path, key: &str, value: Option<Value>) -> Result<(), AppError> {
    let mut current = load(path);
    if !current.is_object() {
        current = Value::Object(Default::default());
    }
    if let Ok(text) = std::fs::read_to_string(path) {
        let trimmed = text.trim();
        if !trimmed.is_empty() && serde_json::from_str::<Value>(trimmed).is_err() {
            let _ = std::fs::copy(path, path.with_extension("json.bak"));
        }
    }
    let obj = current.as_object_mut().expect("上面已保证是对象");
    match value {
        Some(v) => {
            obj.insert(key.to_ascii_lowercase(), v);
        }
        None => {
            obj.remove(&key.to_ascii_lowercase());
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&current)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
```

`mirrors.rs` 顶部实现：

```rust
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
```

`domain/mod.rs` 加 `pub mod mirrors;`。

- [ ] **Step 3: 验证与提交**

Run: `cargo test`（预计 94+9=103）、`cargo check`；`cargo test --test proxy_probe -- --ignored`（真实网络，1 passed）。

```bash
git add src-tauri/src/domain/configfile.rs src-tauri/src/domain/mirrors.rs src-tauri/src/domain/mod.rs src-tauri/tests/proxy_probe.rs
git commit -m "feat: 配置文件写入与镜像源领域（预设推导/git/探针）"
```

---

### Task 2: 镜像源与加速命令（commands/mirrors.rs）

**Files:**
- Create: `src-tauri/src/commands/mirrors.rs`
- Modify: `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 全部；`buckets::git_remote`、`configfile::{resolve,load,set_key}`、`installed`
- Produces（见 Global Constraints 契约）:
  - `MirrorInfo`、`BucketMirrorInfo`、`ProxyTest`（Serialize snake_case）
  - 7 个命令

- [ ] **Step 1: 实现**

创建 `commands/mirrors.rs`：

```rust
use serde::Serialize;
use serde_json::{json, Value};
use tauri::State;

use crate::commands::{context, roots::resolve_active_root};
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

fn resolved_config_path(ctx: &crate::commands::Context, state: &AppState) -> Result<
    (std::path::PathBuf, std::path::PathBuf),
    AppError,
> {
    let root = resolve_active_root(ctx, state).ok_or(AppError::NoRoot)?;
    let resolved = configfile::resolve(&root, &ctx.user_config_home);
    Ok((root, resolved.path))
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

#[tauri::command]
pub async fn set_bucket_source(
    state: State<'_, AppState>,
    bucket: String,
    source: String,
) -> Result<String, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let dir = root.join("buckets").join(&bucket);
    if !dir.is_dir() {
        return Err(AppError::Other(format!("桶不存在: {bucket}")));
    }
    let remote = buckets::git_remote(&dir)
        .ok_or_else(|| AppError::Other(format!("桶不是 git 仓库，无法切换源: {bucket}")))?;
    let url = mirrors::apply_source(&remote, &source, icons::default_proxy())?;
    let dir_text = dir.display().to_string();
    let applied = url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        mirrors::run_git(&dir_text, &["remote", "set-url", "origin", &applied])
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
    let applied = url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        mirrors::run_git(&dir_text, &["remote", "set-url", "origin", &applied])
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
```

`commands/mod.rs` 加 `pub mod mirrors;`；`lib.rs` 注册 7 个命令。

- [ ] **Step 2: 验证与提交**

Run: `cargo test && cargo check` → 全绿（预计 103，无新增纯单测；命令层由 Task 4 真实验收覆盖）。

```bash
git add src-tauri/src/commands/mirrors.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: 镜像源/URL_PROXY/aria2 命令"
```

---

### Task 3: 设置页三区块（前端）

**Files:**
- Modify: `src/api/types.ts`、`src/api/commands.ts`
- Create: `src/components/settings/MirrorSection.vue`、`src/components/settings/AccelerationSection.vue`、`src/components/settings/Aria2Section.vue`
- Modify: `src/pages/SettingsPage.vue`
- Test: `src/__tests__/commands.spec.ts`、`src/__tests__/settings-sections.spec.ts`

**Interfaces:**
- Consumes: Task 2 契约
- Produces: 三个组件（props：`MirrorInfo`/`ProxyTest` 数据；`@changed` 触发 SettingsPage 重载）；SettingsPage 组合

- [ ] **Step 1: 写失败测试**

`commands.spec.ts` 追加（import 7 个包装）：

```ts
    await getMirrorInfo();
    expect(invokeMock).toHaveBeenLastCalledWith("get_mirror_info", undefined);

    await setBucketSource("main", "gitee");
    expect(invokeMock).toHaveBeenLastCalledWith("set_bucket_source", { bucket: "main", source: "gitee" });

    await setScoopSource("official");
    expect(invokeMock).toHaveBeenLastCalledWith("set_scoop_source", { source: "official" });

    await setUrlProxy(null);
    expect(invokeMock).toHaveBeenLastCalledWith("set_url_proxy", { value: null });

    await setProxy("127.0.0.1:7890");
    expect(invokeMock).toHaveBeenLastCalledWith("set_proxy", { value: "127.0.0.1:7890" });

    await setAria2(true, 5, 5);
    expect(invokeMock).toHaveBeenLastCalledWith("set_aria2", { enabled: true, split: 5, maxConnection: 5 });

    await testUrlProxy(null);
    expect(invokeMock).toHaveBeenLastCalledWith("test_url_proxy", { proxy: null });
```

创建 `src/__tests__/settings-sections.spec.ts`：

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  setBucketSource: vi.fn().mockResolvedValue("https://github.com/ScoopInstaller/Main"),
  setScoopSource: vi.fn().mockResolvedValue("https://github.com/ScoopInstaller/Scoop"),
  setUrlProxy: vi.fn().mockResolvedValue(undefined),
  setProxy: vi.fn().mockResolvedValue(undefined),
  setAria2: vi.fn().mockResolvedValue(undefined),
  testUrlProxy: vi.fn(),
  installPackage: vi.fn().mockResolvedValue(undefined),
  getMirrorInfo: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: messageSuccess,
      info: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

import AccelerationSection from "../components/settings/AccelerationSection.vue";
import Aria2Section from "../components/settings/Aria2Section.vue";
import MirrorSection from "../components/settings/MirrorSection.vue";
import { useLogsStore } from "../stores/logs";
import type { MirrorInfo } from "../api/types";

function info(extra: Partial<MirrorInfo> = {}): MirrorInfo {
  return {
    scoop_repo: "https://gitee.com/scoop-installer/scoop",
    scoop_remote: "https://gitee.com/scoop-installer/scoop.git",
    presets: {
      official: "https://github.com/ScoopInstaller/scoop",
      gitee: "https://gitee.com/scoop-installer/scoop",
      proxy: "https://scoop.201704.xyz/https://github.com/ScoopInstaller/scoop",
    },
    url_proxy: null,
    default_url_proxy: "https://scoop.201704.xyz",
    proxy: null,
    buckets: [
      {
        name: "main",
        remote: "https://gitee.com/scoop-installer/Main",
        presets: {
          official: "https://github.com/ScoopInstaller/Main",
          gitee: "https://gitee.com/scoop-installer/Main",
          proxy: "https://scoop.201704.xyz/https://github.com/ScoopInstaller/Main",
        },
      },
      { name: "custom", remote: "https://gitlab.com/x/y", presets: null },
    ],
    aria2_installed: false,
    aria2_enabled: true,
    aria2_split: null,
    aria2_max_connection: null,
    ...extra,
  };
}

describe("设置区块", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.testUrlProxy.mockResolvedValue({
      exit_info: "中国 广东 深圳 电信",
      in_china: true,
      proxy_reachable: true,
      proxy_info: "中国 广东",
    });
  });

  it("镜像区块切换桶源并通知刷新", async () => {
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    const rows = wrapper.findAll(".bucket-row");
    expect(rows.length).toBe(2);
    const select = rows[0].findComponent({ name: "Select" });
    select.vm.$emit("update:value", "official");
    await flushPromises();
    expect(mocks.setBucketSource).toHaveBeenCalledWith("main", "official");
    expect(wrapper.emitted("changed")).toBeTruthy();
    expect(wrapper.text()).toContain("不支持一键切换");
  });

  it("镜像区块切换 scoop 自身源", async () => {
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    const select = wrapper.findComponent({ name: "Select" });
    select.vm.$emit("update:value", "gitee");
    await flushPromises();
    expect(mocks.setScoopSource).toHaveBeenCalledWith("gitee");
  });

  it("加速区块保存自定义 URL_PROXY 与恢复默认", async () => {
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    await wrapper.find('input[placeholder="留空使用默认"]').setValue("https://my.proxy");
    const save = wrapper.findAll("button").find((b) => b.text().includes("保存代理前缀"));
    await save!.trigger("click");
    await flushPromises();
    expect(mocks.setUrlProxy).toHaveBeenCalledWith("https://my.proxy");

    const reset = wrapper.findAll("button").find((b) => b.text().includes("恢复默认"));
    await reset!.trigger("click");
    await flushPromises();
    expect(mocks.setUrlProxy).toHaveBeenLastCalledWith(null);
  });

  it("加速区块连通测试展示出口信息", async () => {
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    const button = wrapper.findAll("button").find((b) => b.text().includes("连通测试"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.testUrlProxy).toHaveBeenCalledWith(null);
    expect(wrapper.text()).toContain("中国 广东 深圳 电信");
    expect(wrapper.text()).toContain("代理前缀可用");
  });

  it("aria2 区块未安装时提供安装按钮并保存参数", async () => {
    const wrapper = mount(Aria2Section, { props: { info: info() } });
    expect(wrapper.text()).toContain("未安装");
    const install = wrapper.findAll("button").find((b) => b.text().includes("安装 aria2"));
    await install!.trigger("click");
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("aria2", null, null, false, expect.any(String));
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(wrapper.emitted("changed")).toBeTruthy();

    mocks.getMirrorInfo.mockResolvedValue(info({ aria2_installed: true }));
    await flushPromises();
    const save = wrapper.findAll("button").find((b) => b.text().includes("保存 aria2 设置"));
    await save!.trigger("click");
    await flushPromises();
    expect(mocks.setAria2).toHaveBeenCalledWith(true, null, null);
  });

  it("任务运行时操作按钮禁用", () => {
    useLogsStore().start("t1", "安装");
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    const save = wrapper.findAll("button").find((b) => b.text().includes("保存代理前缀"));
    expect(save!.attributes("disabled")).toBeDefined();
  });
});
```

Run: `pnpm vitest run src/__tests__/settings-sections.spec.ts` → 红。

- [ ] **Step 2: 实现类型与包装**

`types.ts` 追加：

```ts
export interface MirrorPresets {
  official: string;
  gitee: string;
  proxy: string;
}

export interface BucketMirrorInfo {
  name: string;
  remote: string | null;
  presets: MirrorPresets | null;
}

export interface MirrorInfo {
  scoop_repo: string | null;
  scoop_remote: string | null;
  presets: MirrorPresets | null;
  url_proxy: string | null;
  default_url_proxy: string;
  proxy: string | null;
  buckets: BucketMirrorInfo[];
  aria2_installed: boolean;
  aria2_enabled: boolean;
  aria2_split: number | null;
  aria2_max_connection: number | null;
}

export interface ProxyTest {
  exit_info: string | null;
  in_china: boolean | null;
  proxy_reachable: boolean;
  proxy_info: string | null;
}
```

`commands.ts` 追加（import 类型）：

```ts
export const getMirrorInfo = () => invoke<MirrorInfo>("get_mirror_info");
export const setBucketSource = (bucket: string, source: string) =>
  invoke<string>("set_bucket_source", { bucket, source });
export const setScoopSource = (source: string) => invoke<string>("set_scoop_source", { source });
export const setUrlProxy = (value: string | null) => invoke<void>("set_url_proxy", { value });
export const setProxy = (value: string | null) => invoke<void>("set_proxy", { value });
export const setAria2 = (enabled: boolean, split: number | null, maxConnection: number | null) =>
  invoke<void>("set_aria2", { enabled, split, maxConnection });
export const testUrlProxy = (proxy: string | null) => invoke<ProxyTest>("test_url_proxy", { proxy });
```

- [ ] **Step 3: 实现三个区块组件**

`MirrorSection.vue`（要点：scoop 源下拉 + 桶表格，选项 官方/gitee/201704 代理；presets 为 null 显示「不支持一键切换」；操作成功后 `emit("changed")`；busy 禁用）：

```vue
<template>
  <div class="section">
    <h3>镜像源</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="scoop 仓库当前源">
        {{ info.scoop_remote ?? info.scoop_repo ?? "-" }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="row">
      <n-select
        v-model:value="scoopSource"
        size="small"
        class="source-select"
        :disabled="busy || !info.presets"
        :options="sourceOptions"
        placeholder="选择 scoop 仓库源"
      />
      <n-button size="small" :disabled="busy || !info.presets || !scoopSource" @click="applyScoop">
        应用
      </n-button>
      <span v-if="!info.presets" class="hint">当前仓库不是官方/gitee 已知仓库，不支持一键切换</span>
    </div>

    <n-table :bordered="false" size="small" class="table">
      <thead>
        <tr>
          <th>桶</th>
          <th>当前源</th>
          <th>切换</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="b in info.buckets" :key="b.name" class="bucket-row">
          <td>{{ b.name }}</td>
          <td class="source">{{ b.remote ?? "（非 git）" }}</td>
          <td>
            <template v-if="b.presets">
              <n-select
                :value="null"
                size="tiny"
                class="source-select"
                :disabled="busy"
                :options="sourceOptions"
                placeholder="选择"
                @update:value="(v: string) => applyBucket(b.name, v)"
              />
            </template>
            <span v-else class="hint">不支持一键切换</span>
          </td>
        </tr>
      </tbody>
    </n-table>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NButton, NDescriptions, NDescriptionsItem, NSelect, NTable, useMessage } from "naive-ui";
import { setBucketSource, setScoopSource } from "../../api/commands";
import type { MirrorInfo } from "../../api/types";
import { useLogsStore } from "../../stores/logs";

const props = defineProps<{ info: MirrorInfo }>();
const emit = defineEmits<{ changed: [] }>();
const message = useMessage();
const logs = useLogsStore();
const busy = computed(() => logs.busy);
const scoopSource = ref<string | null>(null);
const sourceOptions = [
  { label: "官方 GitHub", value: "official" },
  { label: "gitee 镜像", value: "gitee" },
  { label: "201704 代理", value: "proxy" },
];

async function applyScoop() {
  if (busy.value || !scoopSource.value) return;
  try {
    const url = await setScoopSource(scoopSource.value);
    message.success(`已切换 scoop 仓库源: ${url}`);
    scoopSource.value = null;
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}

async function applyBucket(name: string, source: string) {
  if (busy.value) return;
  try {
    const url = await setBucketSource(name, source);
    message.success(`桶 ${name} 已切换: ${url}`);
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}
</script>

<style scoped>
.section {
  margin-top: 18px;
}
h3 {
  margin: 0 0 8px;
  font-size: 14px;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}
.table {
  margin-top: 12px;
}
.source-select {
  width: 160px;
}
.source {
  font-family: Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}
.hint {
  color: #999;
  font-size: 12px;
}
</style>
```

`AccelerationSection.vue`（url_proxy 输入/保存/恢复默认、测试、proxy 输入保存、busy 禁用；测试结果展示）：

```vue
<template>
  <div class="section">
    <h3>下载加速（URL_PROXY）</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="当前代理前缀">
        {{ info.url_proxy ?? `默认（${info.default_url_proxy}）` }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="row">
      <n-input v-model:value="proxyInput" size="small" class="input" placeholder="留空使用默认" />
      <n-button size="small" :disabled="busy" @click="saveProxy">保存代理前缀</n-button>
      <n-button size="small" :disabled="busy" @click="resetProxy">恢复默认</n-button>
      <n-button size="small" :loading="testing" :disabled="busy" @click="test">连通测试</n-button>
    </div>
    <n-alert v-if="testResult" :type="testResult.in_china ? 'warning' : 'success'" size="small" class="alert">
      <div>出口 IP：{{ testResult.exit_info ?? "未获取到" }}（{{ testResult.in_china ? "位于中国，建议启用代理" : "不在中国，可直连" }}）</div>
      <div>代理前缀：{{ testResult.proxy_reachable ? `可用（${testResult.proxy_info ?? ""}）` : "不可用" }}</div>
    </n-alert>

    <h3>网络代理（proxy，host:port）</h3>
    <div class="row">
      <n-input v-model:value="plainProxyInput" size="small" class="input" placeholder="例如 127.0.0.1:7890，留空清除" />
      <n-button size="small" :disabled="busy" @click="savePlainProxy">保存</n-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NAlert, NButton, NDescriptions, NDescriptionsItem, NInput, useMessage } from "naive-ui";
import { setProxy, setUrlProxy, testUrlProxy } from "../../api/commands";
import type { MirrorInfo, ProxyTest } from "../../api/types";
import { useLogsStore } from "../../stores/logs";

const props = defineProps<{ info: MirrorInfo }>();
const emit = defineEmits<{ changed: [] }>();
const message = useMessage();
const logs = useLogsStore();
const busy = computed(() => logs.busy);
const proxyInput = ref(props.info.url_proxy ?? "");
const plainProxyInput = ref(props.info.proxy ?? "");
const testing = ref(false);
const testResult = ref<ProxyTest | null>(null);

async function saveProxy() {
  if (busy.value) return;
  try {
    await setUrlProxy(proxyInput.value.trim() === "" ? null : proxyInput.value);
    message.success("已保存 URL_PROXY");
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}

async function resetProxy() {
  if (busy.value) return;
  try {
    await setUrlProxy(null);
    proxyInput.value = "";
    message.success("已恢复默认代理前缀");
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}

async function savePlainProxy() {
  if (busy.value) return;
  try {
    await setProxy(plainProxyInput.value.trim() === "" ? null : plainProxyInput.value);
    message.success("已保存代理设置");
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}

async function test() {
  if (busy.value) return;
  testing.value = true;
  try {
    testResult.value = await testUrlProxy(proxyInput.value.trim() === "" ? null : proxyInput.value);
  } catch (e) {
    message.error(String(e));
  } finally {
    testing.value = false;
  }
}
</script>

<style scoped>
.section {
  margin-top: 18px;
}
h3 {
  margin: 18px 0 8px;
  font-size: 14px;
}
h3:first-child {
  margin-top: 0;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}
.input {
  width: 280px;
}
.alert {
  margin-top: 10px;
}
</style>
```

`Aria2Section.vue`（状态、安装（runTask+installPackage）、enabled 开关、split/max 输入、保存）：

```vue
<template>
  <div class="section">
    <h3>aria2 下载加速</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="安装状态">
        {{ info.aria2_installed ? "已安装" : "未安装" }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="row">
      <n-button v-if="!info.aria2_installed" size="small" :disabled="busy" @click="installAria2">
        安装 aria2
      </n-button>
      <template v-else>
        <n-checkbox v-model:checked="enabled" :disabled="busy">启用 aria2</n-checkbox>
        <n-input-number v-model:value="split" size="small" :min="1" :max="64" :disabled="busy" class="num" />
        <span class="label">分片</span>
        <n-input-number v-model:value="maxConn" size="small" :min="1" :max="64" :disabled="busy" class="num" />
        <span class="label">单服务器连接</span>
        <n-button size="small" :disabled="busy" @click="save">保存 aria2 设置</n-button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NButton, NCheckbox, NDescriptions, NDescriptionsItem, NInputNumber, useMessage } from "naive-ui";
import { installPackage, setAria2 } from "../../api/commands";
import type { MirrorInfo } from "../../api/types";
import { useLogsStore } from "../../stores/logs";
import { useUiStore } from "../../stores/ui";
import { runTask } from "../../utils/tasks";

const props = defineProps<{ info: MirrorInfo }>();
const emit = defineEmits<{ changed: [] }>();
const message = useMessage();
const logs = useLogsStore();
const ui = useUiStore();
const busy = computed(() => logs.busy);
const enabled = ref(props.info.aria2_enabled);
const split = ref<number | null>(props.info.aria2_split ?? 5);
const maxConn = ref<number | null>(props.info.aria2_max_connection ?? 5);

async function installAria2() {
  if (busy.value) return;
  ui.logDrawerOpen = true;
  const record = await runTask("安装 aria2", (taskId) =>
    installPackage("aria2", null, null, false, taskId),
  );
  if (record.success) {
    message.success("aria2 安装完成");
    emit("changed");
  } else {
    message.error(`安装失败（退出码 ${record.code ?? "未知"}），详见任务日志`);
  }
}

async function save() {
  if (busy.value) return;
  try {
    await setAria2(enabled.value, split.value ?? null, maxConn.value ?? null);
    message.success("已保存 aria2 设置");
    emit("changed");
  } catch (e) {
    message.error(String(e));
  }
}
</script>

<style scoped>
.section {
  margin-top: 18px;
}
h3 {
  margin: 0 0 8px;
  font-size: 14px;
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
  flex-wrap: wrap;
}
.num {
  width: 90px;
}
.label {
  color: #888;
  font-size: 12px;
}
</style>
```

`SettingsPage.vue`：script 增加 `import { getMirrorInfo } from "../api/commands";`、`const mirror = ref<MirrorInfo | null>(null);`、`async function loadMirror() { try { mirror.value = await getMirrorInfo(); } catch { mirror.value = null; } }`，`watch(() => roots.active?.path, () => { loadConfig(); loadMirror(); }, { immediate: true });`，模板在「配置文件」之后加：

```vue
    <MirrorSection v-if="mirror" :info="mirror" @changed="loadMirror" />
    <AccelerationSection v-if="mirror" :info="mirror" @changed="loadMirror" />
    <Aria2Section v-if="mirror" :info="mirror" @changed="loadMirror" />
```

- [ ] **Step 4: 运行验证与提交**

Run: 聚焦 spec 全绿；`pnpm test && pnpm build` 全绿。

```bash
git add src/api/types.ts src/api/commands.ts src/components/settings/ src/pages/SettingsPage.vue src/__tests__/commands.spec.ts src/__tests__/settings-sections.spec.ts
git commit -m "feat(ui): 设置页镜像源/URL_PROXY/aria2 区块"
```

---

### Task 4: 真实验收（切换源生效 + 探针）

**Files:** 无代码改动

- [ ] **Step 1: 启动并核对镜像区块**

截图设置页；核对：scoop 源显示当前 gitee remote；4 个桶 remote 与 `git -C ... remote get-url origin` 一致；aria2 显示未安装。

- [ ] **Step 2: 真实切换一个桶的源并复验（结束后恢复原状）**

先记录 `git -C "C:/Users/Administrator/scoop/buckets/main" remote get-url origin`（本机现为 `https://scoop.201704.xyz/https://gitee.com/scoop-installer/Main`）。
在 UI 上按序操作 `main` 桶并每步复验：
1. 「官方 GitHub」→ `https://github.com/ScoopInstaller/Main`
2. 「gitee 镜像」→ `https://gitee.com/scoop-installer/Main`
3. 「201704 代理」→ `https://scoop.201704.xyz/https://gitee.com/scoop-installer/Main`（`proxy` 预设基于当前源加前缀，故可还原为原值；最终 git remote 应与记录完全一致）

- [ ] **Step 3: URL_PROXY 连通测试**

点击「连通测试」→ 截图显示出口 IP 信息与代理前缀可用性；核对与 `Invoke-WebRequest https://ip.glimmer.ltd/0` 内容一致。不保存自定义值（保持默认）。

- [ ] **Step 4: 关闭与报告**

`taskkill scoopui.exe`；报告 `.superpowers/sdd/task-m4b-acceptance.md`（截图、remote 前后值、探针结果、恢复确认）。

---

### Task 5: M4 里程碑评审（M4a + M4b）

**Files:** 无代码改动

- [ ] **Step 1: 生成评审包**

```bash
SKILL_DIR="/c/Users/Administrator/.cache/opencode/packages/superpowers@git+https_/github.com/jnlk-cn/superpowers-cn.git/node_modules/superpowers/skills/subagent-driven-development"
bash "$SKILL_DIR/scripts/review-package" dff950b HEAD
```

- [ ] **Step 2: 分派后端/前端评审（读源码方式）**

后端范围：`domain/versions.rs`、`domain/updates.rs`、`domain/mirrors.rs`、`domain/configfile.rs`、`commands/{updates,mirrors,search}.rs`、`lib.rs`。
前端范围：`stores/apps.ts`、`components/app/AppList.vue`、`AppDetail.vue`、`components/settings/**`、`pages/SettingsPage.vue`、`api/commands.ts`、`api/types.ts`、`__tests__/**`。
重点：版本比较正确性、更新匹配、镜像 URL 推导与注入面（git 参数/URL 校验）、config 原子写、探针阻塞隔离（spawn_blocking）、UI 错误与 busy、M5+ 越界。

---

## Self-Review 记录

- 规格覆盖：设计 §4.5 镜像预设/URL_PROXY/proxy/aria2/特性检测 全部落于 Task 1-3；M4 验收「可切换源并生效」由 Task 4 真实验收覆盖。
- 占位符扫描：无 TBD；所有代码完整。
- 类型一致性：`MirrorPresets`/`BucketMirrorInfo`/`MirrorInfo`/`ProxyTest` 在 Task 2 定义、Task 3 镜像；命令名与参数逐个对齐；`set_aria2` 前端键 `maxConnection` ↔ Rust `max_connection`（Tauri camelCase）已在 commands.spec 断言。
- 已知取舍：非官方/gitee 仓库不支持一键预设（显示提示）；URL_PROXY 只写配置不写入 env；aria2 安装复用 `install_package`（M3 链路）。
- 风险：切换 scoop 自身源会改 `scoop_repo` 配置与 git remote，属真实环境变更——验收仅在用户同意下执行，并记录恢复方式。
