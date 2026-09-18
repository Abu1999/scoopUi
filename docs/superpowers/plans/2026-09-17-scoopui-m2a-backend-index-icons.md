# ScoopUI M2a（后端：清单索引 / 搜索 / 图标管线）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 后端提供离线全量清单索引与搜索（4955 个桶清单，内存索引，一次构建 ~0.5s）、包图标抓取管线（homepage favicon / GitHub owner 头像 → URL_PROXY 改写 → 磁盘缓存 + 负缓存 → base64 data URL），并以 Tauri 命令暴露给前端。

**Architecture:** 纯逻辑放 `domain/index.rs` 与 `domain/icons.rs`（可单测、不依赖 Tauri/网络），网络与命令层薄封装。索引驻留 `AppState`（惰性构建，随根目录变化重建）。**磁盘索引缓存按 YAGNI 延后**（一次构建 0.5s 内，会话内缓存足够；设计文档 §4.3 的磁盘缓存列为后续优化）。图标抓取引入 `reqwest`（阻塞客户端，rustls）。

**Tech Stack:** Rust（Tauri 2）、reqwest 0.12（rustls）、url 2、base64 0.22、serde/serde_json（已有）、thiserror（已有）。

## Global Constraints

- 平台 Windows；面向用户错误中文；代码不加注释；不新增测试框架（沿用 `#[cfg(test)]` + tempfile）
- 新增运行时依赖**仅** `reqwest`、`url`、`base64`（设计文档已列 reqwest；后两者为其配套）
- TDD：先写失败测试再实现；验证门 `cargo check` 0 error、`cargo test` 全绿（`--ignored` 的真实网络测试单独跑）
- 前端契约（M2b 依赖）：
  - `PackageEntry { name, bucket, version, description, homepage, license, icon, deprecated }`（serde snake_case）
  - 命令 `search_packages(query: String, limit: Option<usize>) -> Vec<PackageEntry>`、`refresh_package_index() -> usize`、`fetch_icon(source: String) -> Option<String>`（返回 `data:<mime>;base64,...`；失败 None）
- 图标质量规则：**homepage 指向 github.com/<owner> 时用 `https://github.com/<owner>.png`（机构头像），不用 GitHub 自身 favicon**；其余站点用 `<scheme>://<host>/favicon.ico`；manifest `icon` 字段若存在则直接作为 source
- 代理改写：仅对 `github.com`、`raw.githubusercontent.com`、`objects.githubusercontent.com`、`githubusercontent.com` 主机套用 URL_PROXY 前缀（读 scoop config `url_proxy`，未设置时用 fork 默认 `https://scoop.201704.xyz`）
- 图标缓存目录：`%LOCALAPPDATA%\scoopui\icons\`（`paths::icon_cache_dir` 已有）；负缓存 7 天；单图标上限 512KB；超时 8s；仅接受 `image/*`
- 初始 BASE 提交：`04dba7e`

---

### Task 1: manifest 增加 deprecated 字段 + 索引核心（domain/index.rs）

**Files:**
- Modify: `src-tauri/src/domain/manifest.rs`（`ManifestSummary` 加 `deprecated: bool`）
- Create: `src-tauri/src/domain/index.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod index;`）

**Interfaces:**
- Consumes: `manifest::parse`
- Produces:
  - `ManifestSummary.deprecated: bool`（`"deprecated": true` 时 true）
  - `PackageEntry { name: String, bucket: String, version: String, description: Option<String>, homepage: Option<String>, license: Option<String>, icon: Option<String>, deprecated: bool }`（`Serialize` + `Clone` + `Debug` + `PartialEq`）
  - `index::build(buckets_dir: &Path) -> Vec<PackageEntry>`：遍历 `<bucket>/bucket/**/*.json`（递归），`name` = 文件名去 `.json`，`bucket` = 一级目录名；坏 JSON/缺 version 跳过；按 name 小写升序
  - `index::search(entries: &[PackageEntry], query: &str, limit: usize) -> Vec<PackageEntry>`：大小写不敏感；排名 `0 名称全等 > 1 名称前缀 > 2 名称包含 > 3 描述包含`；同排名按名称小写升序；空查询返回前 limit 个（按名称序）

- [ ] **Step 1: 写失败测试（manifest.deprecated）**

在 `manifest.rs` 的既有测试模块追加：

```rust
    #[test]
    fn reads_deprecated_flag() {
        let m = parse(r#"{"version":"1.0","deprecated":true}"#).unwrap();
        assert!(m.deprecated);
        let m2 = parse(r#"{"version":"1.0"}"#).unwrap();
        assert!(!m2.deprecated);
    }
```

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test manifest::reads_deprecated --lib 2>&1 | grep -E "test result|error"` → 编译失败（字段不存在）——红。

- [ ] **Step 2: 实现 deprecated**

`ManifestSummary` 增加字段 `pub deprecated: bool,`；`Raw` 增加 `deprecated: Option<bool>,`；`parse` 中 `deprecated: raw.deprecated.unwrap_or(false),`。运行上一条命令 → `1 passed`。

- [ ] **Step 3: 写失败测试（index）**

创建 `src-tauri/src/domain/index.rs`（先只写测试 + 最小类型占位会编译失败；按 TDD 先写如下测试，实现留到 Step 4）：

```rust
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
}
```

- [ ] **Step 4: 实现 index.rs**

```rust
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
            let Ok(text) = std::fs::read_to_string(&file) else {
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
                3
            } else if name == q {
                0
            } else if name.starts_with(&q) {
                1
            } else if name.contains(&q) {
                2
            } else if e
                .description
                .as_deref()
                .map(|d| d.to_lowercase().contains(&q))
                .unwrap_or(false)
            {
                3
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
```

在 `domain/mod.rs` 加 `pub mod index;`。

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test index:: --lib 2>&1 | grep -E "test result|^test "` → `3 passed`；`cargo test 2>&1 | grep -E "test result"` 全绿。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/domain/manifest.rs src-tauri/src/domain/index.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 清单索引与搜索（PackageEntry/build/search，含 deprecated）"
```

---

### Task 2: AppState 索引缓存与 Tauri 命令（search_packages / refresh_package_index）

**Files:**
- Modify: `src-tauri/src/state.rs`（索引字段）
- Create: `src-tauri/src/commands/search.rs`
- Modify: `src-tauri/src/commands/mod.rs`（`pub mod search;`）、`src-tauri/src/lib.rs`（注册两个命令）
- Modify: `src-tauri/src/commands/runner.rs` 或其他命令若需要——**不需要**

**Interfaces:**
- Consumes: Task 1 的 `index::{build, search, PackageEntry}`；`resolve_active_root`
- Produces:
  - `AppState.index: Mutex<Option<IndexCache>>`，`IndexCache { root: PathBuf, entries: Vec<PackageEntry> }`
  - 命令 `search_packages(query: String, limit: Option<usize>) -> Result<Vec<PackageEntry>, AppError>`（默认 limit 50；索引惰性构建；根目录变化时自动重建）
  - 命令 `refresh_package_index() -> Result<usize, AppError>`（强制重建并返回条目数）

- [ ] **Step 1: 写失败测试（可测的索引获取逻辑）**

在 `commands/search.rs` 内：把取索引逻辑抽成可测函数：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::index::PackageEntry;
    use crate::state::{AppState, IndexCache};
    use std::path::PathBuf;

    fn entry(name: &str) -> PackageEntry {
        PackageEntry {
            name: name.into(),
            bucket: "main".into(),
            version: "1".into(),
            description: None,
            homepage: None,
            license: None,
            icon: None,
            deprecated: false,
        }
    }

    #[test]
    fn index_cache_reused_for_same_root() {
        let state = AppState::default();
        *state.index.lock().unwrap() = Some(IndexCache {
            root: PathBuf::from("C:\\scoop"),
            entries: vec![entry("git")],
        });
        let got = ensure_index(&state, &PathBuf::from("C:\\scoop"), |_| vec![entry("other")]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "git");
    }

    #[test]
    fn index_rebuilt_when_root_changes() {
        let state = AppState::default();
        *state.index.lock().unwrap() = Some(IndexCache {
            root: PathBuf::from("C:\\a"),
            entries: vec![entry("old")],
        });
        let got = ensure_index(&state, &PathBuf::from("C:\\b"), |_| vec![entry("new")]);
        assert_eq!(got[0].name, "new");
    }
}
```

`ensure_index` 签名约定（Step 2 实现）：

```rust
fn ensure_index(
    state: &AppState,
    root: &std::path::Path,
    builder: impl FnOnce(&std::path::Path) -> Vec<crate::domain::index::PackageEntry>,
) -> std::sync::Arc<Vec<crate::domain::index::PackageEntry>>
```

Run: `cargo test search:: --lib 2>&1 | grep -E "test result|error"` → 红（类型/函数不存在）。

- [ ] **Step 2: 实现 state 字段与 search.rs**

`state.rs`：

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::domain::index::PackageEntry;

pub struct IndexCache {
    pub root: PathBuf,
    pub entries: Vec<PackageEntry>,
}

#[derive(Default)]
pub struct AppState {
    pub active_root: Mutex<Option<PathBuf>>,
    pub tasks: Mutex<HashMap<String, u32>>,
    pub index: Mutex<Option<IndexCache>>,
}
```

`commands/search.rs`（核心逻辑；`Arc<Vec<..>>` 以避免持有锁跨构建）：

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::State;

use crate::commands::{resolve_active_root, context};
use crate::domain::index::{self, PackageEntry};
use crate::error::AppError;
use crate::state::{AppState, IndexCache};

pub(crate) fn ensure_index(
    state: &AppState,
    root: &Path,
    builder: impl FnOnce(&Path) -> Vec<PackageEntry>,
) -> Arc<Vec<PackageEntry>> {
    {
        let guard = state.index.lock().unwrap();
        if let Some(cache) = guard.as_ref() {
            if cache.root == root {
                return Arc::new(cache.entries.clone());
            }
        }
    }
    let entries = builder(root);
    let mut guard = state.index.lock().unwrap();
    *guard = Some(IndexCache {
        root: root.to_path_buf(),
        entries: entries.clone(),
    });
    Arc::new(entries)
}

#[tauri::command(async)]
pub fn search_packages(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<PackageEntry>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = ensure_index(&state, &root, |r| index::build(&r.join("buckets")));
    Ok(index::search(&entries, &query, limit.unwrap_or(50)))
}

#[tauri::command(async)]
pub fn refresh_package_index(state: State<'_, AppState>) -> Result<usize, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = index::build(&root.join("buckets"));
    let count = entries.len();
    *state.index.lock().unwrap() = Some(IndexCache {
        root: root.clone(),
        entries,
    });
    Ok(count)
}
```

注意：`ensure_index` 用 clone 简化（5000 条 clone 有成本但可接受）；实现时若想避免 clone 可改为 `Arc<Vec<..>>` 存在 `IndexCache` 内——**采用后者**：`IndexCache { root, entries: Arc<Vec<PackageEntry>> }`，`ensure_index` 返回 clone 的 Arc（廉价）。相应地把上面测试断言改为 `got[0].name`（Arc deref 可用）。实现者请统一为 Arc 版本并让测试通过。

在 `commands/mod.rs` 加 `pub mod search;`；在 `lib.rs` 的 `generate_handler!` 注册 `commands::search::search_packages, commands::search::refresh_package_index`。

- [ ] **Step 3: 运行测试与编译**

Run: `cargo check 2>&1 | grep -E "error|Finished"`；`cargo test 2>&1 | grep -E "test result"` → 全绿（新增 2 条）。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/state.rs src-tauri/src/commands/search.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: 索引缓存与 search_packages/refresh_package_index 命令"
```

---

### Task 3: 图标管线核心（domain/icons.rs，纯逻辑）

**Files:**
- Create: `src-tauri/src/domain/icons.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod icons;`）
- Modify: `src-tauri/Cargo.toml`（加 `url = "2"`）

**Interfaces:**
- Consumes: `paths::icon_cache_dir`（仅缓存函数用，参数化目录以便测试）
- Produces:
  - `favicon_candidates(source: &str) -> Vec<String>`：`source` 以图片扩展名（`.ico/.png/.jpg/.jpeg/.svg/.webp`，忽略大小写）结尾 → `[source]`；`github.com/<owner>[/...]` → `["https://github.com/<owner>.png"]`；其余 → `["<scheme>://<host>[:port]/favicon.ico"]`；无法解析 → `[]`
  - `rewrite_with_proxy(url: &str, proxy: Option<&str>) -> String`：host ∈ {github.com, raw.githubusercontent.com, objects.githubusercontent.com, githubusercontent.com} 且 proxy 非空 → `{proxy}/{url}`（proxy 去尾斜杠）；否则原样
  - `is_image_content_type(ct: &str) -> bool`（`image/` 前缀，忽略参数与空白）
  - `ext_from_content_type(ct: &str) -> &'static str`（ico/png/jpeg/svg/webp/gif → 扩展名，未知默认 `img`）
  - `cache_key(url: &str) -> String`（FNV-1a 64 位十六进制）
  - `save_icon(dir: &Path, url: &str, bytes: &[u8], mime: &str) -> Result<(), AppError>`：写 `<key>.bin` + `<key>.json`（`{"mime":"...","url":"..."}`）
  - `load_icon(dir: &Path, url: &str) -> Option<(Vec<u8>, String)>`
  - `mark_miss(dir: &Path, url: &str) -> Result<(), AppError>`：写 `<key>.miss`（内容为 UNIX 秒）
  - `is_miss_fresh(dir: &Path, url: &str, max_age: Duration) -> bool`
  - `default_proxy() -> &'static str` = `"https://scoop.201704.xyz"`（fork 默认）

- [ ] **Step 1: 写失败测试**

```rust
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
```

Run: `cargo test icons:: --lib` → 红。

- [ ] **Step 2: 实现 icons.rs**

要求：
- 解析用 `url::Url::parse`；host 用 `host_str()`（注意端口：`favicon.ico` 用 `url.host_str()` + `port()` 重建 origin；可直接用 `format!("{}://{}/favicon.ico", url.scheme(), url[url::Position::BeforeHost..url::Position::AfterPort])`，实现者选一种并用测试锁定端口用例）
- GitHub owner 提取：`url.path_segments().next()`；若 owner 为空或 source 是 `https://github.com/<owner>.png` 形式（已含扩展名）→ 走图片分支（Step 1 测试已覆盖 `.png` 路径）
- `save_icon` 用 `std::fs::write`；`mark_miss` 写 `SystemTime::now().duration_since(UNIX_EPOCH)` 秒数
- `is_miss_fresh`：读文件解析秒数，`now - t < max_age.as_secs()`；解析失败按陈旧处理
- FNV-1a 64：`hash ^= byte; hash = hash.wrapping_mul(0x100000001b3)`，初值 `0xcbf29ce484222325`，输出 `{:016x}`
- 所有函数不加注释、错误经 `AppError::Io`

- [ ] **Step 3: 运行测试确认通过**

Run: `cargo test icons:: --lib 2>&1 | grep -E "test result|^test "` → `5 passed`；全量 `cargo test` 绿。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/domain/icons.rs src-tauri/src/domain/mod.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: 图标管线核心（favicon 候选/代理改写/内容类型/磁盘缓存与负缓存）"
```

---

### Task 4: 图标抓取（reqwest）与 fetch_icon 命令

**Files:**
- Modify: `src-tauri/Cargo.toml`（`reqwest`、`base64`）
- Modify: `src-tauri/src/domain/icons.rs`（HTTP 抓取函数）
- Create: `src-tauri/src/commands/icons.rs`
- Modify: `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`
- Test: `src-tauri/tests/icon_fetch.rs`（`#[ignore]` 真实网络）

**Interfaces:**
- Consumes: Task 3 的全部图标函数；`domain::configfile`（读 url_proxy）；`resolve_active_root`
- Produces:
  - `icons::fetch_remote(url: &str) -> Option<(Vec<u8>, String)>`：阻塞 reqwest，超时 8s，UA `ScoopUI/0.1`，仅 `image/*`，≤512KB
  - `icons::resolve_icon(source: &str, cache_dir: &Path, proxy: Option<&str>) -> Option<String>`：负缓存检查 → 磁盘缓存 → 候选逐个 `rewrite_with_proxy` 后 `fetch_remote` → 成功 `save_icon` 并返回 data URL；全失败 `mark_miss` 返回 None
  - 命令 `fetch_icon(source: String) -> Result<Option<String>, AppError>`：cache_dir = `%LOCALAPPDATA%\scoopui\icons`；proxy = active root 的 config `url_proxy`，否则 `Some(icons::default_proxy())`

- [ ] **Step 1: 加依赖**

```bash
cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo add reqwest --no-default-features --features blocking,rustls-tls 2>&1 | tail -2
cargo add base64 2>&1 | tail -2
```

- [ ] **Step 2: 实现 fetch_remote 与 resolve_icon（含测试钩子）**

`fetch_remote` 与 `resolve_icon` 需要网络，单元测试用**注入获取器**覆盖逻辑（不依赖网络）：

```rust
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
            if save_icon(cache_dir, source, &bytes, &mime).is_err() {
                return None;
            }
            return Some(to_data_url(&bytes, &mime));
        }
    }
    let _ = mark_miss(cache_dir, source);
    None
}

pub fn to_data_url(bytes: &[u8], mime: &str) -> String {
    use base64::Engine;
    format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}
```

新增测试（同文件 tests）：
- `resolve_uses_cache_before_network`：先 `save_icon`，调用 `resolve_with` 且 fetch 闭包 **panic**（若被调用即失败），断言返回 data URL 前缀
- `resolve_negative_cache_blocks_fetch`：`mark_miss` 后 `resolve_with` 中 fetch 闭包 panic，断言 None
- `resolve_falls_back_to_second_candidate`：source 为 github 主页（候选为 owner.png 一个）+ 另一个普通主页（一个候选）——改为构造一个两候选场景：source 为 `https://github.com/o/r`（单候选）→ 用一个自定义 source 无法产生两候选；改为直接测试 fetch 失败→mark_miss→None（闭包返回 None），以及 fetch 成功→保存并可再次从缓存读取
- `to_data_url` 编码测试（`[0u8]` → `data:image/png;base64,AA==`）

`fetch_remote`：

```rust
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
    let bytes = resp.bytes().ok()?.to_vec();
    if bytes.len() > MAX {
        return None;
    }
    Some((bytes, ct))
}
```

`resolve_icon` = `resolve_with(source, cache_dir, proxy, fetch_remote)`。

- [ ] **Step 3: 命令层**

`commands/icons.rs`：

```rust
use tauri::State;

use crate::commands::{context, resolve_active_root};
use crate::domain::{configfile, icons};
use crate::error::AppError;
use crate::paths;
use crate::state::AppState;

#[tauri::command(async)]
pub fn fetch_icon(state: State<'_, AppState>, source: String) -> Result<Option<String>, AppError> {
    let ctx = context();
    let cache_dir = paths::icon_cache_dir(&ctx.local_appdata);
    let proxy = resolve_active_root(&ctx, &state)
        .map(|root| configfile::resolve(&root, &ctx.user_config_home))
        .and_then(|cfg| {
            configfile::load(&cfg.path)
                .get("url_proxy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| icons::default_proxy().to_string());
    Ok(icons::resolve_icon(&source, &cache_dir, Some(&proxy)))
}
```

注意：`Context` 目前没有 `local_appdata` 字段——**在 `commands/mod.rs` 的 `Context` 增加 `pub local_appdata: PathBuf`**（从 `LOCALAPPDATA` 读取，缺失时用 `appdata` 兜底），并在 `context()` 中填充；`Context` 是测试里手工构造的，加字段会让既有测试编译失败——**同步更新 `commands/roots.rs` 测试中的 Context 构造**（加 `local_appdata: tmp.path().to_path_buf()`）。

`configfile::load` 的 url_proxy 读取注意键名小写（config 写入已统一小写；读取时 `get("url_proxy")`）。

在 `commands/mod.rs` 加 `pub mod icons;`；`lib.rs` 注册 `commands::icons::fetch_icon`。

- [ ] **Step 4: 集成测试（真实网络，#[ignore]）**

`src-tauri/tests/icon_fetch.rs`：

```rust
use std::path::PathBuf;

use scoopui_lib::domain::icons;

#[test]
#[ignore = "需要真实网络；用 cargo test --test icon_fetch -- --ignored 运行"]
fn fetches_real_favicon() {
    let tmp = tempfile::tempdir().unwrap();
    let out = icons::resolve_icon("https://www.python.org", tmp.path(), None);
    assert!(out.as_deref().unwrap_or("").starts_with("data:image/"));
    let cached = icons::load_icon(tmp.path(), "https://www.python.org");
    assert!(cached.is_some());
}
```

（`tempfile` 已是 dev-dependency；`icons::resolve_icon` 与 `load_icon` 需 `pub`。）

- [ ] **Step 5: 验证与提交**

Run: `cargo test 2>&1 | grep -E "test result"`（全绿）；`cargo test --test icon_fetch -- --ignored 2>&1 | grep -E "test result|^test "`（1 passed，真实 python.org favicon）；`cargo check` 0 error。

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/domain/icons.rs src-tauri/src/commands/icons.rs src-tauri/src/commands/mod.rs src-tauri/src/commands/roots.rs src-tauri/src/lib.rs src-tauri/tests/icon_fetch.rs
git commit -m "feat: 图标抓取（reqwest 阻塞客户端 + 缓存/负缓存）与 fetch_icon 命令"
```

---

## M2a 验收清单（人工）

- [ ] `cargo test` 全绿（预计 47 + 3 index + 2 search + 5 icons + 约 4 resolve/data-url ≈ 61）
- [ ] `cargo test --test icon_fetch -- --ignored` 通过（真实抓取 python.org favicon）
- [ ] `cargo check` 无 error
- [ ] 契约稳定：`search_packages` / `refresh_package_index` / `fetch_icon` 与 Global Constraints 一致
- [ ] 索引构建耗时实测（`refresh_package_index` 返回条目数 ≈ 4955，命令耗时记录在报告）

## Self-Review 记录

- 规格覆盖：设计文档 §4.3（本地索引 + 更新检测属 M4）、§4.4（图标优先级链：manifest.icon → homepage favicon → 首字母兜底；懒加载由前端按需调用；缓存 + 负缓存）；磁盘索引缓存有意延后（YAGNI，理由见 Architecture）。
- 占位符扫描：无 TBD；所有函数含实现代码与测试。
- 类型一致性：`PackageEntry` 字段在 Task 1 定义，Task 2 命令与 M2b 契约复用；`IndexCache` 在 Task 2 与 state.rs 同步；`Context.local_appdata` 的新增在 Task 4 明确要求同步更新既有测试。
- 已知风险：`reqwest` 阻塞客户端在 `#[tauri::command(async)]` 线程池中执行，单次抓取最长 8s 不阻塞主线程；`resolve_with` 的注入式测试避免网络依赖。
