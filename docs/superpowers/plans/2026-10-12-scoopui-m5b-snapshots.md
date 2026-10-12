# ScoopUI M5b 备份/恢复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现快照备份与半自动恢复：创建/列出/删除快照（config + 软件源 + 已装清单 + 配置集），恢复配置与软件源，并列出缺失应用供逐条/全部重装。

**Architecture:** 新增 `domain/snapshots.rs`（存储与恢复逻辑）与 `commands/snapshots.rs`（6 个命令）；前端新增 `stores/snapshots.ts` 与 `pages/BackupPage.vue`，缺失软件源与缺失应用的重装由前端复用 `runTask` 逐条串行编排（失败即停），复用 M5a 的步进模式。

**Tech Stack:** Rust（Tauri 2、serde_json、tempfile）、Vue 3 + TypeScript + Naive UI + Pinia + Vitest。无新增依赖。

**Spec:** `docs/superpowers/specs/2026-10-12-scoopui-m5b-m7-remaining-design.md` §2

## Global Constraints

- 契约加法：仅新增命令与字段；不改既有命令参数/返回结构
- 代码不加注释；不加依赖；UI 文案全中文（术语用「软件源」，不用「桶」）
- 快照存储：`%APPDATA%\scoopui\snapshots\<epoch秒>\`；`meta.json` 最后写入（未完成的目录被 list 跳过）
- 恢复安全：config 写回前先备份当前为 `config.json.bak`，tmp+rename 原子写
- 验证门：`cargo test` 全绿；`pnpm test` 全绿；`pnpm build`（vue-tsc）无错
- 初始 BASE 提交：`4f72e8e`
- 当前基线：Rust 120 passed；前端 139 passed（20 文件）
- Windows 环境：Rust 命令前先 `export PATH="$USERPROFILE/.cargo/bin:$PATH"`

## File Structure

**后端**
- `src-tauri/src/paths.rs`（改）：`snapshots_dir`
- `src-tauri/src/domain/snapshots.rs`（新）：类型、存储（list/load/create/delete）、恢复（plan/restore_config/restore_buckets）
- `src-tauri/src/domain/mod.rs`（改）：注册模块
- `src-tauri/src/domain/profiles.rs`（改）：`is_remote_url` 改 `pub(crate)` 供复用
- `src-tauri/src/commands/snapshots.rs`（新）：6 个命令
- `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`（改）：注册

**前端**
- `src/api/types.ts`、`src/api/commands.ts`（改）：快照类型与命令封装
- `src/stores/snapshots.ts`（新）：列表/计划/恢复编排
- `src/pages/BackupPage.vue`（新）：备份页
- `src/components/layout/AppShell.vue`（改）：`backup` 路由

**测试**
- 后端：`domain/snapshots.rs`、`paths.rs` 内联测试
- 前端：`src/__tests__/snapshots.spec.ts`、`backup-page.spec.ts`（新）；`commands.spec.ts`、`appshell.spec.ts`（改）

---

### Task 1: 快照存储基础（paths + domain 类型与 CRUD）

**Files:**
- Modify: `src-tauri/src/paths.rs`
- Create: `src-tauri/src/domain/snapshots.rs`
- Modify: `src-tauri/src/domain/mod.rs`
- Modify: `src-tauri/src/domain/profiles.rs:202`（`fn is_remote_url` → `pub(crate) fn is_remote_url`）

**Interfaces:**
- Consumes: `profiles::{BucketRef, now_epoch, is_remote_url}`、`domain::{buckets::BucketInfo, installed::InstalledApp}`、`root::normalized`
- Produces: `paths::snapshots_dir(&Path) -> PathBuf`；`snapshots::{SnapshotMeta, SnapshotApp, SnapshotSummary, validate_id, list, load_meta, load_buckets, load_apps, create, delete, snapshot_dir}`

- [ ] **Step 1: 写失败测试**

`src-tauri/src/paths.rs` 的 `joins_expected_subpaths` 追加：

```rust
        assert_eq!(snapshots_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\scoopui\\snapshots"));
```

新建 `src-tauri/src/domain/snapshots.rs`（先放类型与测试模块；实现由 Step 3 补全；`domain/mod.rs` 同步注册 `pub mod snapshots;`，此时编译红）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::buckets::BucketInfo;
    use crate::domain::installed::InstalledApp;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn bucket(name: &str, source: Option<&str>) -> BucketInfo {
        BucketInfo {
            name: name.to_string(),
            source: source.map(|s| s.to_string()),
            is_git: source.is_some(),
            manifest_count: 0,
        }
    }

    fn app(name: &str, bucket: Option<&str>, version: &str, global: bool) -> InstalledApp {
        InstalledApp {
            name: name.to_string(),
            version: version.to_string(),
            bucket: bucket.map(|s| s.to_string()),
            architecture: None,
            hold: false,
            updated: None,
            description: None,
            homepage: None,
            icon: None,
            depends: vec![],
            persist: vec![],
            bin: vec![],
            app_dir: format!("C:\\scoop\\apps\\{name}"),
            global,
        }
    }

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let t = tmp();
        let root = t.path().join("scoop");
        std::fs::create_dir_all(root.join("buckets")).unwrap();
        let config = root.join("config.json");
        std::fs::write(&config, r#"{"url_proxy":"https://scoop.201704.xyz"}"#).unwrap();
        let profiles = t.path().join("profiles");
        std::fs::create_dir_all(&profiles).unwrap();
        std::fs::write(profiles.join("工作.json"), "{}").unwrap();
        (t, root, config)
    }

    #[test]
    fn validate_id_rejects_non_numeric() {
        assert!(validate_id("1758614400").is_ok());
        for bad in ["", "..", "a1", "1/2", "1 2"] {
            assert!(validate_id(bad).is_err(), "{bad:?} 应被拒绝");
        }
    }

    #[test]
    fn create_writes_all_files_and_meta_last() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let profiles = t.path().join("profiles");
        let summary = create(
            &dir,
            &root,
            &config,
            &[bucket("main", Some("https://github.com/ScoopInstaller/Main"))],
            &[app("git", Some("main"), "2.55.0.5", false), app("7zip", Some("main"), "26.00", true)],
            &profiles,
        )
        .unwrap();
        assert!(summary.id.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(summary.app_count, 2);
        assert_eq!(summary.bucket_count, 1);
        assert_eq!(summary.profile_count, 1);
        assert!(summary.created > 0);
        let target = snapshot_dir(&dir, &summary.id);
        for f in ["meta.json", "config.json", "buckets.json", "apps.json"] {
            assert!(target.join(f).is_file(), "{f} 应存在");
        }
        assert!(target.join("profiles").join("工作.json").is_file());
        let meta = load_meta(&dir, &summary.id).unwrap();
        assert!(meta.config_present);
        assert_eq!(meta.root, root.display().to_string());

        let buckets = load_buckets(&dir, &summary.id).unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].source.as_deref(), Some("https://github.com/ScoopInstaller/Main"));
        let apps = load_apps(&dir, &summary.id).unwrap();
        assert_eq!(apps.len(), 2);
        assert!(apps.iter().any(|a| a.name == "7zip" && a.global));
    }

    #[test]
    fn list_skips_corrupt_and_sorts_desc() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let a = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        let b = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        assert_ne!(a.id, b.id);
        std::fs::create_dir_all(dir.join("not-numeric")).unwrap();
        std::fs::write(dir.join("not-numeric").join("meta.json"), "{}").unwrap();
        let list = list(&dir);
        assert_eq!(list.len(), 2);
        assert!(list[0].created >= list[1].created);
    }

    #[test]
    fn delete_removes_dir_and_missing_errors() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        delete(&dir, &s.id).unwrap();
        assert!(!snapshot_dir(&dir, &s.id).exists());
        assert!(delete(&dir, &s.id).is_err());
        assert!(load_meta(&dir, &s.id).is_err());
    }

    #[test]
    fn create_without_config_or_profiles_is_ok() {
        let (t, root, _config) = fixture();
        let dir = t.path().join("snapshots");
        let summary = create(&dir, &root, &root.join("missing.json"), &[], &[], &t.path().join("nope")).unwrap();
        assert_eq!(summary.profile_count, 0);
        let meta = load_meta(&dir, &summary.id).unwrap();
        assert!(!meta.config_present);
        assert!(!snapshot_dir(&dir, &summary.id).join("config.json").exists());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib snapshots::`
Expected: 编译失败（类型与函数未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/paths.rs` 追加：

```rust
pub fn snapshots_dir(appdata: &Path) -> PathBuf {
    settings_dir(appdata).join("snapshots")
}
```

`src-tauri/src/domain/profiles.rs:202`：`fn is_remote_url(source: &str) -> bool` 改为 `pub(crate) fn is_remote_url(source: &str) -> bool`。

`src-tauri/src/domain/snapshots.rs` 的 `use` 区与实现（放在测试模块之前）：

```rust
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::buckets::BucketInfo;
use crate::domain::installed::InstalledApp;
use crate::domain::profiles::{is_remote_url, now_epoch, BucketRef};
use crate::error::AppError;

pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub version: u32,
    pub created: u64,
    pub root: String,
    pub config_path: String,
    pub config_present: bool,
    pub app_count: usize,
    pub bucket_count: usize,
    pub profile_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotApp {
    pub name: String,
    pub bucket: Option<String>,
    pub version: String,
    #[serde(default)]
    pub hold: bool,
    #[serde(default)]
    pub global: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SnapshotSummary {
    pub id: String,
    pub created: u64,
    pub root: String,
    pub app_count: usize,
    pub bucket_count: usize,
    pub profile_count: usize,
}

pub fn validate_id(id: &str) -> Result<(), AppError> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Other(format!("快照 ID 非法: {id}")));
    }
    Ok(())
}

pub fn snapshot_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(id)
}

pub fn list(dir: &Path) -> Vec<SnapshotSummary> {
    let mut out: Vec<SnapshotSummary> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let Ok(meta) = load_meta(dir, &id) else {
            continue;
        };
        out.push(SnapshotSummary {
            id,
            created: meta.created,
            root: meta.root,
            app_count: meta.app_count,
            bucket_count: meta.bucket_count,
            profile_count: meta.profile_count,
        });
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

pub fn load_meta(dir: &Path, id: &str) -> Result<SnapshotMeta, AppError> {
    validate_id(id)?;
    let text = std::fs::read_to_string(snapshot_dir(dir, id).join("meta.json"))
        .map_err(|_| AppError::Other(format!("快照不存在: {id}")))?;
    serde_json::from_str(&text).map_err(|_| AppError::Other(format!("快照文件损坏: {id}")))
}

pub fn load_buckets(dir: &Path, id: &str) -> Result<Vec<BucketRef>, AppError> {
    load_json(dir, id, "buckets.json")
}

pub fn load_apps(dir: &Path, id: &str) -> Result<Vec<SnapshotApp>, AppError> {
    load_json(dir, id, "apps.json")
}

fn load_json<T: serde::de::DeserializeOwned>(dir: &Path, id: &str, file: &str) -> Result<T, AppError> {
    validate_id(id)?;
    let text = std::fs::read_to_string(snapshot_dir(dir, id).join(file))
        .map_err(|_| AppError::Other(format!("快照不存在: {id}")))?;
    serde_json::from_str(&text).map_err(|_| AppError::Other(format!("快照文件损坏: {id}")))
}

pub fn create(
    dir: &Path,
    root: &Path,
    config_path: &Path,
    buckets: &[BucketInfo],
    apps: &[InstalledApp],
    profiles_src: &Path,
) -> Result<SnapshotSummary, AppError> {
    std::fs::create_dir_all(dir)?;
    let mut created = now_epoch();
    let mut id = created.to_string();
    while snapshot_dir(dir, &id).exists() {
        created += 1;
        id = created.to_string();
    }
    let target = snapshot_dir(dir, &id);
    std::fs::create_dir_all(&target)?;

    let config_present = config_path.is_file();
    if config_present {
        std::fs::copy(config_path, target.join("config.json"))?;
    }
    let bucket_refs: Vec<BucketRef> = buckets
        .iter()
        .map(|b| BucketRef {
            name: b.name.clone(),
            source: b.source.clone(),
        })
        .collect();
    std::fs::write(target.join("buckets.json"), serde_json::to_string_pretty(&bucket_refs)?)?;
    let snapshot_apps: Vec<SnapshotApp> = apps
        .iter()
        .map(|a| SnapshotApp {
            name: a.name.clone(),
            bucket: a.bucket.clone(),
            version: a.version.clone(),
            hold: a.hold,
            global: a.global,
        })
        .collect();
    std::fs::write(target.join("apps.json"), serde_json::to_string_pretty(&snapshot_apps)?)?;

    let mut profile_count = 0usize;
    if profiles_src.is_dir() {
        let dst = target.join("profiles");
        std::fs::create_dir_all(&dst)?;
        if let Ok(entries) = std::fs::read_dir(profiles_src) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_json = path
                    .extension()
                    .map(|e| e.eq_ignore_ascii_case("json"))
                    .unwrap_or(false);
                if path.is_file() && is_json && std::fs::copy(&path, dst.join(entry.file_name())).is_ok() {
                    profile_count += 1;
                }
            }
        }
    }

    let meta = SnapshotMeta {
        version: SNAPSHOT_VERSION,
        created,
        root: root.display().to_string(),
        config_path: config_path.display().to_string(),
        config_present,
        app_count: apps.len(),
        bucket_count: buckets.len(),
        profile_count,
    };
    std::fs::write(target.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;

    Ok(SnapshotSummary {
        id,
        created,
        root: meta.root,
        app_count: meta.app_count,
        bucket_count: meta.bucket_count,
        profile_count: meta.profile_count,
    })
}

pub fn delete(dir: &Path, id: &str) -> Result<(), AppError> {
    validate_id(id)?;
    let target = snapshot_dir(dir, id);
    if !target.is_dir() {
        return Err(AppError::Other(format!("快照不存在: {id}")));
    }
    std::fs::remove_dir_all(target)?;
    Ok(())
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 125 passed（120 + 5：paths 断言不新增用例，snapshots 新增 5 个用例）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/paths.rs src-tauri/src/domain/snapshots.rs src-tauri/src/domain/mod.rs src-tauri/src/domain/profiles.rs
git commit -m "feat: 快照存储基础（创建/列表/读取/删除）"
```

---

### Task 2: 恢复计划与写回（plan / restore_config / restore_buckets）

**Files:**
- Modify: `src-tauri/src/domain/snapshots.rs`

**Interfaces:**
- Consumes: Task 1 的存储函数；`root::normalized`、`mirrors::run_git`、`buckets::git_remote`、`profiles::is_remote_url`
- Produces: `RestorePlan`、`RestoreBucketsResult`、`plan`、`restore_config`、`restore_buckets`

- [ ] **Step 1: 写失败测试**

`src-tauri/src/domain/snapshots.rs` 的 `mod tests` 追加 5 个用例：

```rust
    #[test]
    fn plan_reports_missing_apps_buckets_and_reset() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(
            &dir,
            &root,
            &config,
            &[
                bucket("main", Some("https://github.com/ScoopInstaller/Main")),
                bucket("extras", Some("https://gitee.com/scoop-installer/Extras")),
            ],
            &[
                app("git", Some("main"), "2.55.0.5", false),
                app("7zip", Some("main"), "26.00", true),
            ],
            &t.path().join("none"),
        )
        .unwrap();
        let current_buckets = vec![
            bucket("main", Some("https://github.com/ScoopInstaller/Main")),
            bucket("java", None),
        ];
        let current_apps = vec![app("git", Some("main"), "2.55.0.5", false)];
        let p = plan(&dir, &s.id, &root, &config, &current_buckets, &current_apps).unwrap();
        assert!(p.root_matches);
        assert!(p.config_present);
        assert_eq!(p.config_path, config.display().to_string());
        assert_eq!(p.reset_buckets.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["main"]);
        assert_eq!(p.missing_buckets.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["extras"]);
        assert_eq!(p.missing_apps.len(), 1);
        assert!(p.missing_apps[0].global);
        assert_eq!(p.missing_apps[0].name, "7zip");
    }

    #[test]
    fn plan_flags_root_mismatch() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        let p = plan(&dir, &s.id, &t.path().join("other"), &config, &[], &[]).unwrap();
        assert!(!p.root_matches);
    }

    #[test]
    fn restore_config_backs_up_and_writes() {
        let (t, root, config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(&dir, &root, &config, &[], &[], &t.path().join("none")).unwrap();
        std::fs::write(&config, r#"{"old":true}"#).unwrap();
        restore_config(&dir, &s.id, &config).unwrap();
        let restored = std::fs::read_to_string(&config).unwrap();
        assert!(restored.contains("url_proxy"));
        let bak = std::fs::read_to_string(root.join("config.json.bak")).unwrap();
        assert!(bak.contains("old"));
        assert!(!root.join("config.json.tmp").exists());
    }

    #[test]
    fn restore_config_without_config_errors() {
        let (t, root, _config) = fixture();
        let dir = t.path().join("snapshots");
        let s = create(&dir, &root, &root.join("missing.json"), &[], &[], &t.path().join("none")).unwrap();
        let err = restore_config(&dir, &s.id, &root.join("config.json")).unwrap_err().to_string();
        assert!(err.contains("未包含配置文件"), "实际: {err}");
    }

    #[test]
    fn restore_buckets_sets_remote_and_reports_missing() {
        let (t, root, config) = fixture();
        let main = root.join("buckets").join("main");
        std::fs::create_dir_all(&main).unwrap();
        crate::domain::mirrors::run_git(&main.display().to_string(), &["init", "-q"]).unwrap();
        crate::domain::mirrors::run_git(
            &main.display().to_string(),
            &["remote", "add", "origin", "https://github.com/ScoopInstaller/Main"],
        )
        .unwrap();
        std::fs::create_dir_all(root.join("buckets").join("notgit")).unwrap();

        let dir = t.path().join("snapshots");
        let s = create(
            &dir,
            &root,
            &config,
            &[
                bucket("main", Some("https://gitee.com/scoop-installer/Main")),
                bucket("extras", Some("https://gitee.com/scoop-installer/Extras")),
                bucket("notgit", Some("https://gitee.com/scoop-installer/NotGit")),
                bucket("localonly", None),
            ],
            &[],
            &t.path().join("none"),
        )
        .unwrap();
        let result = restore_buckets(&dir, &s.id, &root).unwrap();
        assert_eq!(result.reset, vec!["main"]);
        assert_eq!(
            result.missing.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            vec!["extras", "localonly"]
        );
        assert_eq!(result.skipped, vec!["notgit"]);
        assert_eq!(
            crate::domain::buckets::git_remote(&main).as_deref(),
            Some("https://gitee.com/scoop-installer/Main")
        );
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib snapshots::`
Expected: 编译失败（`plan` / `restore_config` / `restore_buckets` / `RestorePlan` / `RestoreBucketsResult` 未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/domain/snapshots.rs` 在 `SnapshotSummary` 之后追加类型：

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestorePlan {
    pub root: String,
    pub root_matches: bool,
    pub config_path: String,
    pub config_present: bool,
    pub reset_buckets: Vec<BucketRef>,
    pub missing_buckets: Vec<BucketRef>,
    pub missing_apps: Vec<SnapshotApp>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RestoreBucketsResult {
    pub reset: Vec<String>,
    pub missing: Vec<BucketRef>,
    pub skipped: Vec<String>,
}
```

在 `delete` 之后追加：

```rust
pub fn plan(
    dir: &Path,
    id: &str,
    current_root: &Path,
    current_config_path: &Path,
    current_buckets: &[BucketInfo],
    current_apps: &[InstalledApp],
) -> Result<RestorePlan, AppError> {
    let meta = load_meta(dir, id)?;
    let mut buckets = load_buckets(dir, id)?;
    let mut apps = load_apps(dir, id)?;
    let lower = |s: &str| s.to_lowercase();
    let current_names: Vec<String> = current_buckets.iter().map(|b| lower(&b.name)).collect();
    let mut reset_buckets: Vec<BucketRef> = Vec::new();
    let mut missing_buckets: Vec<BucketRef> = Vec::new();
    buckets.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    for b in buckets {
        if current_names.contains(&lower(&b.name)) {
            if b.source.as_deref().map(is_remote_url).unwrap_or(false) {
                reset_buckets.push(b);
            }
        } else {
            missing_buckets.push(b);
        }
    }
    let app_key = |name: &str, global: bool| (lower(name), global);
    let current_keys: Vec<(String, bool)> = current_apps.iter().map(|a| app_key(&a.name, a.global)).collect();
    apps.retain(|a| !current_keys.contains(&app_key(&a.name, a.global)));
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.global.cmp(&b.global))
    });
    Ok(RestorePlan {
        root: meta.root.clone(),
        root_matches: crate::domain::root::normalized(current_root)
            == crate::domain::root::normalized(Path::new(&meta.root)),
        config_path: current_config_path.display().to_string(),
        config_present: meta.config_present,
        reset_buckets,
        missing_buckets,
        missing_apps: apps,
    })
}

pub fn restore_config(dir: &Path, id: &str, config_path: &Path) -> Result<(), AppError> {
    let meta = load_meta(dir, id)?;
    if !meta.config_present {
        return Err(AppError::Other("该快照未包含配置文件".to_string()));
    }
    let text = std::fs::read_to_string(snapshot_dir(dir, id).join("config.json"))
        .map_err(|_| AppError::Other(format!("快照文件损坏: {id}")))?;
    serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|_| AppError::Other(format!("快照文件损坏: {id}")))?;
    if config_path.is_file() {
        let _ = std::fs::copy(config_path, config_path.with_extension("json.bak"));
    }
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = config_path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, config_path)?;
    Ok(())
}

fn safe_bucket_name(name: &str) -> bool {
    !name.is_empty()
        && name == name.trim_end_matches([' ', '.'])
        && !name
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control())
}

pub fn restore_buckets(dir: &Path, id: &str, root: &Path) -> Result<RestoreBucketsResult, AppError> {
    let buckets = load_buckets(dir, id)?;
    let mut reset: Vec<String> = Vec::new();
    let mut missing: Vec<BucketRef> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for b in buckets {
        if !safe_bucket_name(&b.name) {
            skipped.push(b.name);
            continue;
        }
        let local = root.join("buckets").join(&b.name);
        if !local.is_dir() {
            missing.push(b);
            continue;
        }
        let Some(source) = b.source.as_deref().filter(|s| is_remote_url(s)) else {
            skipped.push(b.name);
            continue;
        };
        if crate::domain::buckets::git_remote(&local).is_none() {
            skipped.push(b.name);
            continue;
        }
        crate::domain::mirrors::run_git(
            &local.display().to_string(),
            &["remote", "set-url", "origin", source],
        )?;
        reset.push(b.name);
    }
    Ok(RestoreBucketsResult { reset, missing, skipped })
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 130 passed（125 + 5）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/snapshots.rs
git commit -m "feat: 快照恢复计划与写回"
```

---

### Task 3: 快照命令层与注册

**Files:**
- Create: `src-tauri/src/commands/snapshots.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1/2 的 domain 函数；`commands::{context, scan_installed}`、`roots::resolve_active_root`
- Produces: 命令 `list_snapshots` / `create_snapshot` / `delete_snapshot` / `restore_snapshot_plan` / `restore_snapshot_config` / `restore_snapshot_buckets`

- [ ] **Step 1: 实现命令层**（薄适配层，逻辑已由 Task 1/2 单测覆盖；验证方式为编译 + 全量测试）

新建 `src-tauri/src/commands/snapshots.rs`：

```rust
use std::path::PathBuf;

use tauri::State;

use crate::commands::{context, roots::resolve_active_root, scan_installed};
use crate::domain::buckets;
use crate::domain::configfile;
use crate::domain::snapshots::{self, RestoreBucketsResult, RestorePlan, SnapshotSummary};
use crate::error::AppError;
use crate::paths;
use crate::state::AppState;

fn snapshots_dir(ctx: &crate::commands::Context) -> PathBuf {
    paths::snapshots_dir(&ctx.appdata)
}

#[tauri::command(async)]
pub fn list_snapshots() -> Result<Vec<SnapshotSummary>, AppError> {
    let ctx = context();
    Ok(snapshots::list(&snapshots_dir(&ctx)))
}

#[tauri::command(async)]
pub fn create_snapshot(state: State<'_, AppState>) -> Result<SnapshotSummary, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    let bucket_list = buckets::scan(&root.join("buckets"));
    let apps = scan_installed(&ctx, &root);
    snapshots::create(
        &snapshots_dir(&ctx),
        &root,
        &config_path,
        &bucket_list,
        &apps,
        &paths::profiles_dir(&ctx.appdata),
    )
}

#[tauri::command(async)]
pub fn delete_snapshot(id: String) -> Result<(), AppError> {
    let ctx = context();
    snapshots::delete(&snapshots_dir(&ctx), &id)
}

#[tauri::command(async)]
pub fn restore_snapshot_plan(state: State<'_, AppState>, id: String) -> Result<RestorePlan, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    let bucket_list = buckets::scan(&root.join("buckets"));
    let apps = scan_installed(&ctx, &root);
    snapshots::plan(&snapshots_dir(&ctx), &id, &root, &config_path, &bucket_list, &apps)
}

#[tauri::command(async)]
pub fn restore_snapshot_config(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    snapshots::restore_config(&snapshots_dir(&ctx), &id, &config_path)
}

#[tauri::command(async)]
pub fn restore_snapshot_buckets(state: State<'_, AppState>, id: String) -> Result<RestoreBucketsResult, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    snapshots::restore_buckets(&snapshots_dir(&ctx), &id, &root)
}
```

`src-tauri/src/commands/mod.rs` 的 `pub mod` 列表插入（字母序，`search` 之后）：

```rust
pub mod snapshots;
```

`src-tauri/src/lib.rs` 的 `invoke_handler` 中 `commands::profiles::apply_profile_settings` 之后追加（注意前一行的逗号）：

```rust
            commands::snapshots::list_snapshots,
            commands::snapshots::create_snapshot,
            commands::snapshots::delete_snapshot,
            commands::snapshots::restore_snapshot_plan,
            commands::snapshots::restore_snapshot_config,
            commands::snapshots::restore_snapshot_buckets
```

- [ ] **Step 2: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 130 passed（无新增测试，编译与既有测试全绿）

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/commands/snapshots.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: 快照命令层与注册"
```

---

### Task 4: 前端 API 与 snapshots store

**Files:**
- Modify: `src/api/types.ts`
- Modify: `src/api/commands.ts`
- Create: `src/stores/snapshots.ts`
- Create: `src/__tests__/snapshots.spec.ts`
- Modify: `src/__tests__/commands.spec.ts`

**Interfaces:**
- Consumes: Task 3 的命令；`runTask`；`apps`/`buckets` stores
- Produces: `SnapshotSummary` / `SnapshotApp` / `RestorePlan` / `RestoreBucketsResult` 类型；6 个命令封装；`useSnapshotsStore`（load/select/refreshPlan/create/remove/restoreConfigAndBuckets/installApp/installAllMissing）

- [ ] **Step 1: 写失败测试**

`src/__tests__/commands.spec.ts`：`import` 列表补 `createSnapshot, deleteSnapshot, listSnapshots, restoreSnapshotBuckets, restoreSnapshotConfig, restoreSnapshotPlan`，并在 `describe("commands 包装")` 内追加：

```ts
  it("M5b 快照命令名与参数符合后端契约", async () => {
    await listSnapshots();
    expect(invokeMock).toHaveBeenLastCalledWith("list_snapshots", undefined);

    await createSnapshot();
    expect(invokeMock).toHaveBeenLastCalledWith("create_snapshot", undefined);

    await deleteSnapshot("100");
    expect(invokeMock).toHaveBeenLastCalledWith("delete_snapshot", { id: "100" });

    await restoreSnapshotPlan("100");
    expect(invokeMock).toHaveBeenLastCalledWith("restore_snapshot_plan", { id: "100" });

    await restoreSnapshotConfig("100");
    expect(invokeMock).toHaveBeenLastCalledWith("restore_snapshot_config", { id: "100" });

    await restoreSnapshotBuckets("100");
    expect(invokeMock).toHaveBeenLastCalledWith("restore_snapshot_buckets", { id: "100" });
  });
```

新建 `src/__tests__/snapshots.spec.ts`：

```ts
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listSnapshots: vi.fn(),
  createSnapshot: vi.fn(),
  deleteSnapshot: vi.fn(),
  restoreSnapshotPlan: vi.fn(),
  restoreSnapshotConfig: vi.fn(),
  restoreSnapshotBuckets: vi.fn(),
  bucketAdd: vi.fn(),
  installPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { useLogsStore } from "../stores/logs";
import { useSnapshotsStore } from "../stores/snapshots";
import type { RestorePlan } from "../api/types";

const summary = {
  id: "100",
  created: 1758614400,
  root: "C:\\scoop",
  app_count: 2,
  bucket_count: 1,
  profile_count: 1,
};

const planFixture: RestorePlan = {
  root: "C:\\scoop",
  root_matches: true,
  config_path: "C:\\scoop\\config.json",
  config_present: true,
  reset_buckets: [{ name: "main", source: "https://gitee.com/scoop-installer/Main" }],
  missing_buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  missing_apps: [{ name: "7zip", bucket: "main", version: "26.00", hold: false, global: false }],
};

describe("snapshots store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("load 拉取列表并加载计划", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    const store = useSnapshotsStore();
    await store.load();
    expect(store.selectedId).toBe("100");
    expect(store.selected?.root).toBe("C:\\scoop");
    expect(store.plan?.missing_apps[0].name).toBe("7zip");
    expect(store.loading).toBe(false);
  });

  it("load 失败时 error 可读且列表为空", async () => {
    mocks.listSnapshots.mockRejectedValue("读取快照目录失败");
    const store = useSnapshotsStore();
    await store.load();
    expect(store.list).toEqual([]);
    expect(store.error).toContain("读取快照目录失败");
  });

  it("restoreConfigAndBuckets 恢复配置并添加缺失软件源", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    mocks.restoreSnapshotConfig.mockResolvedValue(undefined);
    mocks.restoreSnapshotBuckets.mockResolvedValue({
      reset: ["main"],
      missing: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
      skipped: [],
    });
    mocks.bucketAdd.mockResolvedValue(undefined);
    const store = useSnapshotsStore();
    await store.load();
    const running = store.restoreConfigAndBuckets();
    await flushPromises();
    expect(mocks.restoreSnapshotConfig).toHaveBeenCalledWith("100");
    expect(mocks.restoreSnapshotBuckets).toHaveBeenCalledWith("100");
    expect(mocks.bucketAdd).toHaveBeenCalledWith(
      "extras",
      "https://gitee.com/scoop-installer/Extras",
      expect.any(String),
    );
    const taskId = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(await running).toBe(true);
    expect(store.steps.map((s) => s.status)).toEqual(["success"]);
  });

  it("installAllMissing 失败即停并标记剩余", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue({
      ...planFixture,
      missing_apps: [
        { name: "7zip", bucket: "main", version: "26.00", hold: false, global: false },
        { name: "git", bucket: "main", version: "2.55.0.5", hold: false, global: false },
      ],
    });
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useSnapshotsStore();
    await store.load();
    const running = store.installAllMissing();
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(await running).toBe(false);
    expect(store.steps[0].status).toBe("failed");
    expect(store.steps[1].status).toBe("skipped");
    expect(mocks.installPackage).toHaveBeenCalledTimes(1);
    expect(store.restoreError).toContain("退出码 1");
  });

  it("create 后选中新快照并刷新", async () => {
    mocks.createSnapshot.mockResolvedValue({ ...summary, id: "200", created: 1758614500 });
    mocks.listSnapshots.mockResolvedValue([
      { ...summary, id: "200", created: 1758614500 },
      summary,
    ]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    const store = useSnapshotsStore();
    await store.create();
    expect(mocks.createSnapshot).toHaveBeenCalled();
    expect(store.selectedId).toBe("200");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/snapshots.spec.ts src/__tests__/commands.spec.ts`
Expected: FAIL（`stores/snapshots.ts` 与命令封装不存在，导入失败）

- [ ] **Step 3: 实现**

`src/api/types.ts` 末尾追加：

```ts
export interface SnapshotSummary {
  id: string;
  created: number;
  root: string;
  app_count: number;
  bucket_count: number;
  profile_count: number;
}

export interface SnapshotApp {
  name: string;
  bucket: string | null;
  version: string;
  hold: boolean;
  global: boolean;
}

export interface RestorePlan {
  root: string;
  root_matches: boolean;
  config_path: string;
  config_present: boolean;
  reset_buckets: BucketRef[];
  missing_buckets: BucketRef[];
  missing_apps: SnapshotApp[];
}

export interface RestoreBucketsResult {
  reset: string[];
  missing: BucketRef[];
  skipped: string[];
}
```

`src/api/commands.ts` 末尾追加（`import type` 列表补 `RestoreBucketsResult, RestorePlan, SnapshotSummary`）：

```ts
export const listSnapshots = () => invoke<SnapshotSummary[]>("list_snapshots");
export const createSnapshot = () => invoke<SnapshotSummary>("create_snapshot");
export const deleteSnapshot = (id: string) => invoke<void>("delete_snapshot", { id });
export const restoreSnapshotPlan = (id: string) => invoke<RestorePlan>("restore_snapshot_plan", { id });
export const restoreSnapshotConfig = (id: string) => invoke<void>("restore_snapshot_config", { id });
export const restoreSnapshotBuckets = (id: string) => invoke<RestoreBucketsResult>("restore_snapshot_buckets", { id });
```

新建 `src/stores/snapshots.ts`：

```ts
import { defineStore } from "pinia";
import {
  bucketAdd,
  createSnapshot,
  deleteSnapshot,
  installPackage,
  listSnapshots,
  restoreSnapshotBuckets,
  restoreSnapshotConfig,
  restoreSnapshotPlan,
} from "../api/commands";
import type { RestoreBucketsResult, RestorePlan, SnapshotApp, SnapshotSummary } from "../api/types";
import { runTask } from "../utils/tasks";
import { useAppsStore } from "./apps";
import { useBucketsStore } from "./buckets";
import { useUiStore } from "./ui";

export type InstallStepStatus = "pending" | "running" | "success" | "failed" | "skipped";

export interface InstallStep {
  key: string;
  label: string;
  status: InstallStepStatus;
}

export const useSnapshotsStore = defineStore("snapshots", {
  state: () => ({
    list: [] as SnapshotSummary[],
    selectedId: null as string | null,
    plan: null as RestorePlan | null,
    loading: false,
    error: null as string | null,
    restoring: false,
    steps: [] as InstallStep[],
    restoreError: null as string | null,
  }),
  getters: {
    selected(state): SnapshotSummary | null {
      return state.list.find((s) => s.id === state.selectedId) ?? null;
    },
  },
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.list = await listSnapshots();
        if (!this.list.some((s) => s.id === this.selectedId)) {
          this.selectedId = this.list[0]?.id ?? null;
        }
      } catch (e) {
        this.list = [];
        this.error = String(e);
      }
      this.loading = false;
      if (!this.selectedId) {
        this.plan = null;
        return;
      }
      try {
        await this.loadPlan();
      } catch (e) {
        this.error = String(e);
      }
    },
    async loadPlan() {
      const id = this.selectedId;
      if (!id) return;
      const plan = await restoreSnapshotPlan(id);
      if (this.selectedId !== id) return;
      this.plan = plan;
    },
    async select(id: string) {
      this.selectedId = id;
      this.error = null;
      this.restoreError = null;
      this.steps = [];
      try {
        await this.loadPlan();
      } catch (e) {
        this.error = String(e);
      }
    },
    async refreshPlan() {
      if (!this.selectedId) return;
      this.plan = await restoreSnapshotPlan(this.selectedId);
    },
    async create() {
      const s = await createSnapshot();
      this.selectedId = s.id;
      await this.load();
    },
    async remove() {
      if (!this.selectedId) return;
      await deleteSnapshot(this.selectedId);
      this.selectedId = null;
      await this.load();
    },
    async restoreConfigAndBuckets(): Promise<boolean> {
      const id = this.selectedId;
      if (!id) return false;
      this.restoreError = null;
      this.steps = [];
      this.restoring = true;
      useUiStore().logDrawerOpen = true;
      try {
        await restoreSnapshotConfig(id);
      } catch (e) {
        this.restoreError = String(e);
        this.restoring = false;
        return false;
      }
      let result: RestoreBucketsResult;
      try {
        result = await restoreSnapshotBuckets(id);
      } catch (e) {
        this.restoreError = String(e);
        this.restoring = false;
        return false;
      }
      this.steps = result.missing
        .filter((b) => b.source)
        .map((b) => ({ key: `bucket:${b.name}`, label: `添加软件源 ${b.name}`, status: "pending" as const }));
      let failed = false;
      for (const step of this.steps) {
        if (failed) {
          step.status = "skipped";
          continue;
        }
        step.status = "running";
        const bucket = result.missing.find((b) => step.key === `bucket:${b.name}`)!;
        const record = await runTask(step.label, (taskId) => bucketAdd(bucket.name, bucket.source, taskId));
        if (record.success) {
          step.status = "success";
        } else {
          step.status = "failed";
          this.restoreError = `步骤失败（退出码 ${record.code ?? "未知"}）`;
          failed = true;
        }
      }
      this.restoring = false;
      if (!failed) {
        await Promise.all([useAppsStore().load(), useBucketsStore().load(), this.refreshPlan()]);
      }
      return !failed;
    },
    async installApp(app: SnapshotApp): Promise<boolean> {
      if (!this.selectedId) return false;
      const record = await runTask(
        `安装 ${app.name}${app.global ? "（全局）" : ""}`,
        (taskId) => installPackage(app.name, app.bucket, null, app.global, taskId),
      );
      if (record.success) {
        await Promise.all([useAppsStore().load(), this.refreshPlan()]);
        return true;
      }
      this.restoreError = `步骤失败（退出码 ${record.code ?? "未知"}）`;
      return false;
    },
    async installAllMissing(): Promise<boolean> {
      const plan = this.plan;
      if (!plan || plan.missing_apps.length === 0) return false;
      this.restoreError = null;
      this.restoring = true;
      this.steps = plan.missing_apps.map((a) => ({
        key: `install:${a.name}@${a.global}`,
        label: `安装 ${a.name}${a.global ? "（全局）" : ""}`,
        status: "pending" as const,
      }));
      useUiStore().logDrawerOpen = true;
      let failed = false;
      for (const step of this.steps) {
        if (failed) {
          step.status = "skipped";
          continue;
        }
        step.status = "running";
        const app = plan.missing_apps.find((a) => step.key === `install:${a.name}@${a.global}`)!;
        const record = await runTask(step.label, (taskId) =>
          installPackage(app.name, app.bucket, null, app.global, taskId),
        );
        if (record.success) {
          step.status = "success";
        } else {
          step.status = "failed";
          this.restoreError = `步骤失败（退出码 ${record.code ?? "未知"}）`;
          failed = true;
        }
      }
      this.restoring = false;
      if (!failed) {
        await Promise.all([useAppsStore().load(), this.refreshPlan()]);
      }
      return !failed;
    },
  },
});
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/snapshots.spec.ts src/__tests__/commands.spec.ts`
Expected: PASS（snapshots 5 + commands 既有 7 + 1 = 8）

- [ ] **Step 5: 提交**

```bash
git add src/api/types.ts src/api/commands.ts src/stores/snapshots.ts src/__tests__/snapshots.spec.ts src/__tests__/commands.spec.ts
git commit -m "feat(ui): 快照 API 与 store"
```

---

### Task 5: 备份页面与路由

**Files:**
- Create: `src/pages/BackupPage.vue`
- Modify: `src/components/layout/AppShell.vue`
- Create: `src/__tests__/backup-page.spec.ts`
- Modify: `src/__tests__/appshell.spec.ts`

**Interfaces:**
- Consumes: Task 4 的 `useSnapshotsStore`
- Produces: `backup` 页路由；创建/恢复/删除/安装交互

- [ ] **Step 1: 写失败测试**

新建 `src/__tests__/backup-page.spec.ts`：

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  listSnapshots: vi.fn(),
  createSnapshot: vi.fn(),
  deleteSnapshot: vi.fn(),
  restoreSnapshotPlan: vi.fn(),
  restoreSnapshotConfig: vi.fn(),
  restoreSnapshotBuckets: vi.fn(),
  bucketAdd: vi.fn(),
  installPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
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

import BackupPage from "../pages/BackupPage.vue";
import { useLogsStore } from "../stores/logs";
import { useSnapshotsStore } from "../stores/snapshots";
import type { RestorePlan } from "../api/types";

const summary = {
  id: "100",
  created: 1758614400,
  root: "C:\\scoop",
  app_count: 2,
  bucket_count: 1,
  profile_count: 1,
};

const planFixture: RestorePlan = {
  root: "C:\\scoop",
  root_matches: true,
  config_path: "C:\\scoop\\config.json",
  config_present: true,
  reset_buckets: [{ name: "main", source: "https://gitee.com/scoop-installer/Main" }],
  missing_buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  missing_apps: [{ name: "7zip", bucket: "main", version: "26.00", hold: false, global: false }],
};

describe("BackupPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("首次挂载自动加载列表并渲染恢复计划", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(BackupPage, { global: { plugins: [pinia] } });
    await flushPromises();
    expect(mocks.listSnapshots).toHaveBeenCalled();
    expect(wrapper.text()).toContain("C:\\scoop");
    expect(wrapper.text()).toContain("缺失应用");
    expect(wrapper.text()).toContain("7zip");
    expect(wrapper.text()).toContain("缺失软件源");
  });

  it("创建快照调用命令", async () => {
    mocks.listSnapshots.mockResolvedValue([]);
    mocks.createSnapshot.mockResolvedValue(summary);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(BackupPage, { global: { plugins: [pinia] } });
    await flushPromises();
    const btn = wrapper.findAll("button").find((b) => b.text().includes("创建快照"));
    await btn!.trigger("click");
    await flushPromises();
    expect(mocks.createSnapshot).toHaveBeenCalled();
    expect(messageSuccess).toHaveBeenCalled();
  });

  it("恢复配置与软件源调用命令并添加缺失源", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    mocks.restoreSnapshotConfig.mockResolvedValue(undefined);
    mocks.restoreSnapshotBuckets.mockResolvedValue({
      reset: ["main"],
      missing: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
      skipped: [],
    });
    mocks.bucketAdd.mockResolvedValue(undefined);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(BackupPage, { global: { plugins: [pinia] } });
    await flushPromises();
    const btn = wrapper.findAll("button").find((b) => b.text() === "恢复配置与软件源");
    await btn!.trigger("click");
    await flushPromises();
    expect(mocks.restoreSnapshotConfig).toHaveBeenCalledWith("100");
    expect(mocks.bucketAdd).toHaveBeenCalledWith(
      "extras",
      "https://gitee.com/scoop-installer/Extras",
      expect.any(String),
    );
    const taskId = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(messageSuccess).toHaveBeenCalled();
  });

  it("缺失应用安装失败提示错误", async () => {
    mocks.listSnapshots.mockResolvedValue([summary]);
    mocks.restoreSnapshotPlan.mockResolvedValue(planFixture);
    mocks.installPackage.mockResolvedValue(undefined);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(BackupPage, { global: { plugins: [pinia] } });
    await flushPromises();
    const install = wrapper.findAll("button").find((b) => b.text() === "安装");
    await install!.trigger("click");
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
  });
});
```

`src/__tests__/appshell.spec.ts`：`stubs` 对象加 `BackupPage: true,`，并追加路由用例：

```ts
  it("备份页渲染 BackupPage 而非占位", () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useUiStore().setPage("backup");
    const wrapper = mount(AppShell, { global: { plugins: [pinia], stubs } });
    expect(wrapper.find("backup-page-stub").exists()).toBe(true);
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/backup-page.spec.ts src/__tests__/appshell.spec.ts`
Expected: FAIL（`BackupPage.vue` 不存在；AppShell 仍渲染占位）

- [ ] **Step 3: 实现**

新建 `src/pages/BackupPage.vue`：

```vue
<template>
  <div class="page">
    <div class="side">
      <div class="side-head">
        <n-button size="small" @click="doCreate">创建快照</n-button>
      </div>
      <n-alert v-if="snapshots.error" type="error" size="small" class="alert">{{ snapshots.error }}</n-alert>
      <div class="list">
        <div v-if="snapshots.list.length === 0" class="empty">还没有快照，点击「创建快照」保存当前状态</div>
        <div
          v-for="s in snapshots.list"
          :key="s.id"
          class="row"
          :class="{ active: s.id === snapshots.selectedId }"
          @click="snapshots.select(s.id)"
        >
          <div class="name">{{ formatTime(s.created) }}</div>
          <div class="meta">{{ s.root }}</div>
          <div class="meta">应用 {{ s.app_count }} · 软件源 {{ s.bucket_count }} · 配置集 {{ s.profile_count }}</div>
        </div>
      </div>
    </div>

    <div class="detail">
      <div v-if="!snapshots.selected" class="empty">选择一个快照查看详情</div>
      <template v-else>
        <div class="head">
          <div>
            <div class="title">{{ formatTime(snapshots.selected.created) }}</div>
            <div class="sub">来自根目录 {{ snapshots.selected.root }}</div>
          </div>
          <div class="actions">
            <n-button size="small" :disabled="busy" @click="doRestore">恢复配置与软件源</n-button>
            <n-popconfirm
              :positive-button-props="{ disabled: busy, type: 'error' }"
              @positive-click="doDelete"
            >
              <template #trigger>
                <n-button size="small" type="error" :disabled="busy">删除</n-button>
              </template>
              确定删除快照 {{ formatTime(snapshots.selected.created) }}？
            </n-popconfirm>
          </div>
        </div>

        <n-alert v-if="snapshots.plan && !snapshots.plan.root_matches" type="warning" size="small" class="alert">
          快照来自 {{ snapshots.plan.root }}，当前根目录不同；恢复将写入当前根目录的配置
        </n-alert>

        <div v-if="snapshots.plan" class="section">
          <div class="section-title">恢复计划</div>
          <div class="diff-line">
            配置文件：{{ snapshots.plan.config_present ? snapshots.plan.config_path : "快照未包含配置" }}
          </div>
          <div class="diff-line">
            重设软件源（{{ snapshots.plan.reset_buckets.length }}）：{{
              snapshots.plan.reset_buckets.map((b) => b.name).join("、") || "无"
            }}
          </div>
          <div class="diff-line">
            缺失软件源（{{ snapshots.plan.missing_buckets.length }}）：{{
              snapshots.plan.missing_buckets.map((b) => b.name).join("、") || "无"
            }}
          </div>
        </div>

        <div v-if="snapshots.plan" class="section">
          <div class="section-title">缺失应用（{{ snapshots.plan.missing_apps.length }}）</div>
          <div v-if="snapshots.plan.missing_apps.length === 0" class="hint">没有缺失应用</div>
          <div v-for="a in snapshots.plan.missing_apps" :key="`${a.name}@${a.global}`" class="line">
            <span class="mono">{{ a.name }}</span>
            <span v-if="a.global" class="badge">全局</span>
            <span class="dim">{{ a.version }} · {{ a.bucket ?? "未知软件源" }}</span>
            <n-button size="tiny" :disabled="busy" @click="doInstall(a)">安装</n-button>
          </div>
          <n-button
            v-if="snapshots.plan.missing_apps.length > 0"
            size="small"
            class="install-all"
            :disabled="busy"
            :loading="snapshots.restoring"
            @click="doInstallAll"
          >
            全部重装
          </n-button>
        </div>

        <div v-if="snapshots.steps.length > 0" class="section">
          <div class="section-title">执行进度</div>
          <div v-for="s in snapshots.steps" :key="s.key" class="step" :class="s.status">
            <span class="status-text">{{ statusText(s.status) }}</span>
            <span>{{ s.label }}</span>
          </div>
        </div>
        <n-alert v-if="snapshots.restoreError" type="error" size="small" class="alert">
          {{ snapshots.restoreError }}
        </n-alert>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from "vue";
import { NAlert, NButton, NPopconfirm, useMessage } from "naive-ui";
import type { SnapshotApp } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useSnapshotsStore } from "../stores/snapshots";
import type { InstallStepStatus } from "../stores/snapshots";

const snapshots = useSnapshotsStore();
const logs = useLogsStore();
const message = useMessage();
const busy = computed(() => logs.busy);

onMounted(() => {
  snapshots.load();
});

function formatTime(sec: number): string {
  if (!sec) return "-";
  return new Date(sec * 1000).toLocaleString();
}

function statusText(s: InstallStepStatus): string {
  const map: Record<InstallStepStatus, string> = {
    pending: "待执行",
    running: "执行中",
    success: "成功",
    failed: "失败",
    skipped: "待执行",
  };
  return map[s];
}

async function doCreate() {
  try {
    await snapshots.create();
    message.success("已创建快照");
  } catch (e) {
    message.error(String(e));
  }
}

async function doDelete() {
  try {
    await snapshots.remove();
    message.success("已删除快照");
  } catch (e) {
    message.error(String(e));
  }
}

async function doRestore() {
  const ok = await snapshots.restoreConfigAndBuckets();
  if (ok) {
    message.success("配置与软件源已恢复");
  } else {
    message.error(snapshots.restoreError ?? "恢复失败，详见任务日志");
  }
}

async function doInstall(app: SnapshotApp) {
  const ok = await snapshots.installApp(app);
  if (ok) {
    message.success(`已安装 ${app.name}`);
  } else {
    message.error(snapshots.restoreError ?? "安装失败，详见任务日志");
  }
}

async function doInstallAll() {
  const ok = await snapshots.installAllMissing();
  if (ok) {
    message.success("缺失应用已全部安装");
  } else {
    message.error(snapshots.restoreError ?? "安装失败，详见任务日志");
  }
}
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.side {
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid #efeff5;
}
.side-head {
  display: flex;
  gap: 8px;
  padding: 10px;
}
.alert {
  margin: 0 10px 8px;
}
.list {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.row {
  padding: 8px 12px;
  cursor: pointer;
  border-bottom: 1px solid #f5f5f7;
}
.row:hover {
  background: #f5f7fa;
}
.row.active {
  background: #eaf3ff;
}
.row .name {
  font-size: 14px;
}
.row .meta {
  font-size: 12px;
  color: #888;
  word-break: break-all;
}
.detail {
  flex: 1;
  min-width: 0;
  padding: 16px;
  overflow: auto;
}
.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 12px;
}
.title {
  font-size: 18px;
  font-weight: 600;
}
.sub {
  color: #888;
  font-size: 12px;
  margin-top: 4px;
  word-break: break-all;
}
.actions {
  display: flex;
  gap: 8px;
}
.section {
  margin-top: 18px;
}
.section-title {
  font-size: 13px;
  color: #666;
  margin-bottom: 6px;
}
.diff-line {
  font-size: 13px;
  color: #444;
  margin-bottom: 4px;
  word-break: break-all;
}
.line {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 13px;
  margin-bottom: 4px;
}
.mono {
  font-family: Consolas, monospace;
}
.dim {
  color: #888;
  font-size: 12px;
  word-break: break-all;
}
.badge {
  font-size: 11px;
  color: #18a058;
  border: 1px solid #18a058;
  border-radius: 3px;
  padding: 0 4px;
}
.hint {
  color: #999;
  font-size: 12px;
}
.install-all {
  margin-top: 8px;
}
.step {
  display: flex;
  gap: 8px;
  font-size: 13px;
  margin-bottom: 4px;
}
.step .status-text {
  width: 48px;
  flex-shrink: 0;
  color: #888;
}
.step.success .status-text {
  color: #18a058;
}
.step.failed .status-text {
  color: #d03050;
}
.step.running .status-text {
  color: #2080f0;
}
.empty {
  padding: 24px;
  color: #999;
  text-align: center;
}
</style>
```

`src/components/layout/AppShell.vue`：

1) 模板在 `ProfilesPage` 之后插入：

```vue
        <BackupPage v-else-if="ui.activePage === 'backup'" />
```

2) `script setup` 导入：

```ts
import BackupPage from "../../pages/BackupPage.vue";
```

3) `placeholders` 删除 `backup` 行，只保留：

```ts
const placeholders: Record<string, string> = {
  migrate: "环境迁移（M6：注册表扫描与接管）",
};
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/backup-page.spec.ts src/__tests__/appshell.spec.ts`
Expected: PASS（新增 4 + 1 个用例 + 既有用例全绿）

Run: `pnpm test`
Expected: 150 passed（139 + commands 1 + snapshots 5 + backup-page 4 + appshell 1）

- [ ] **Step 5: 提交**

```bash
git add src/pages/BackupPage.vue src/components/layout/AppShell.vue src/__tests__/backup-page.spec.ts src/__tests__/appshell.spec.ts
git commit -m "feat(ui): 备份页面与路由"
```

---

### Task 6: 全量验证

**Files:** 无新增；只做验证与收尾

- [ ] **Step 1: 后端全量测试**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 130 passed、0 failed

- [ ] **Step 2: 前端全量测试与构建**

Run: `pnpm test`
Expected: 150 passed

Run: `pnpm build`
Expected: `vue-tsc` 无错误、vite build 成功

- [ ] **Step 3: 手工验收清单（用户在 Windows 本机执行 `pnpm tauri dev`）**

1. 备份页：点击「创建快照」→ 列表出现新快照（时间/根路径/计数正确）
2. 修改 config（例如在设置页改一个下载加速项）与软件源 remote → 快照详情显示恢复计划（重设源/缺失应用/缺失源）
3. 「恢复配置与软件源」→ 配置写回（可打开 config.json 核对）、当前配置被备份为 `config.json.bak`、软件源 remote 被重设
4. 缺失应用「安装」单条成功；「全部重装」逐条串行、失败即停、剩余显示「待执行」
5. 删除快照需确认且列表刷新
6. 无快照时显示空态引导

- [ ] **Step 4: 修复与提交（仅当验证发现问题）**

若发现问题：修复 → 回到 Step 1 重新验证 → 提交（`fix: ...`）。全部通过则无需额外提交。

---

## Self-Review 记录

- 规格覆盖：§2.1 存储（Task 1）、§2.2 命令（Task 3）、§2.3 恢复流程（Task 2 后端 + Task 4 store 编排 + Task 5 页面）、§2.4 备份页（Task 5）、§5 测试策略（各任务内联 + Task 6）。
- 占位符扫描：无 TBD；所有步骤含完整代码与精确命令。
- 类型一致性：`SnapshotMeta`/`SnapshotApp`/`SnapshotSummary`/`RestorePlan`/`RestoreBucketsResult` 在 Task 1/2 定义，Task 3 命令签名、Task 4 前端类型与 store、Task 5 页面逐一对齐；`BucketRef` 复用 `profiles::BucketRef`；任务步骤 key 规则（`bucket:name`、`install:name@global`）在 store 内自洽。
- 已知取舍：快照 ID 使用 epoch 秒（同秒冲突自动 +1）；`restore_buckets` 中途 git 失败即整体报错（不做部分回滚）；恢复不自动重装应用（半自动决策）。


