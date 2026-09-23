# ScoopUI M5a 配置集 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现配置集（Profile）：另存/导入/导出/重命名/删除、与当前状态差异对比、一键应用（settings → 加桶 → 装缺失应用 → 可选卸载多余），并让已安装页与更新检测覆盖全局应用（SCOOP_GLOBAL）。

**Architecture:** 后端新增 `domain/profiles.rs`（类型/校验/存储/快照/导入解析/差异/设置写入）与 `commands/profiles.rs`（9 个命令）；`installed::scan` 增加 `global` 参数，`commands::scan_installed` 统一合并用户+全局应用供 apps/updates/profiles 复用。前端新增 `utils/appkey.ts`（`name@global` 键）、`stores/profiles.ts`、`pages/ProfilesPage.vue`；应用流程由前端编排既有命令（`bucket_add`/`install_package`/`uninstall_package`/`apply_profile_settings`），逐条串行、失败即停。

**Tech Stack:** Rust（Tauri 2、serde_json、tempfile）、Vue 3 + TypeScript + Naive UI + Pinia + Vitest。无新增依赖。

**Spec:** `docs/superpowers/specs/2026-09-23-scoopui-m5a-profiles-design.md`

## Global Constraints

- 契约加法：仅新增字段（`InstalledApp.global`、`UpdateInfo.global`）与新增命令；不改既有命令参数/返回结构
- UI 文案全中文；代码不加注释；不加依赖
- 后端验证门：`cargo test` 全绿；前端：`pnpm test` 全绿、`pnpm build`（vue-tsc）无错
- 初始 BASE 提交：`c84457a`
- Windows 环境：Rust 命令前先 `export PATH="$USERPROFILE/.cargo/bin:$PATH"`
- 当前基线：Rust 104 passed；前端 121 passed（18 文件）
- settings 白名单（固定顺序）：`url_proxy`、`proxy`、`aria2-enabled`、`aria2-split`、`aria2-max-connection-per-server`

## File Structure

**后端**
- `src-tauri/src/paths.rs`（改）：新增 `profiles_dir`
- `src-tauri/src/domain/profiles.rs`（新）：类型、名称校验、存储 CRUD、snapshot、parse_import、diff、apply_settings
- `src-tauri/src/domain/mod.rs`（改）：注册 `profiles` 模块
- `src-tauri/src/domain/installed.rs`（改）：`scan` 增加 `global` 参数与字段
- `src-tauri/src/domain/updates.rs`（改）：`UpdateInfo.global`
- `src-tauri/src/domain/root.rs`（改）：`scan` 调用补 `false`
- `src-tauri/src/commands/mod.rs`（改）：`Context::global_dir()` + `scan_installed()`
- `src-tauri/src/commands/apps.rs`（改）：合并用户+全局应用
- `src-tauri/src/commands/updates.rs`（改）：合并列表参与更新检测
- `src-tauri/src/commands/profiles.rs`（新）：9 个命令
- `src-tauri/src/lib.rs`（改）：注册命令

**前端**
- `src/api/types.ts`（改）：`InstalledApp.global`、`UpdateInfo.global`、Profile 系类型
- `src/api/commands.ts`（改）：9 个 profile 命令封装
- `src/utils/appkey.ts`（新）：`appKey`
- `src/stores/apps.ts`（改）：`selectedKey` + 键控 updates
- `src/components/app/AppList.vue`（改）：全局徽标 + key 传参
- `src/components/app/AppDetail.vue`（改）：全局标记 + 操作传 `global`
- `src/pages/InstalledPage.vue`（改）：`selectedKey`
- `src/stores/profiles.ts`（新）：列表/详情/差异/应用编排
- `src/pages/ProfilesPage.vue`（新）：左列表+右详情+四个弹窗
- `src/components/layout/AppShell.vue`（改）：`profiles` 路由

**测试**
- 后端：`domain/installed.rs`、`domain/updates.rs`、`domain/profiles.rs`、`paths.rs` 内联测试
- 前端：`src/__tests__/stores.spec.ts`、`app-components.spec.ts`、`app-ops.spec.ts`、`appshell.spec.ts`（改）；`src/__tests__/profiles.spec.ts`、`profiles-page.spec.ts`（新）

---

### Task 1: 后端全局应用支持

**Files:**
- Modify: `src-tauri/src/domain/installed.rs`
- Modify: `src-tauri/src/domain/updates.rs`
- Modify: `src-tauri/src/domain/root.rs:92`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/commands/apps.rs`
- Modify: `src-tauri/src/commands/updates.rs`

**Interfaces:**
- Produces: `installed::scan(apps_dir: &Path, global: bool) -> Vec<InstalledApp>`；`InstalledApp.global: bool`；`UpdateInfo.global: bool`；`Context::global_dir(&self) -> PathBuf`；`commands::scan_installed(ctx: &Context, root: &Path) -> Vec<InstalledApp>`

- [ ] **Step 1: 写失败测试**

`src-tauri/src/domain/installed.rs`：把既有测试中 3 处 `scan` 调用补 `false` 参数（`scan(apps, false)`、`scan(tmp.path(), false)`、`scan(&tmp.path().join("nope"), false)`），并在 `mod tests` 内追加：

```rust
    #[test]
    fn marks_global_scan() {
        let tmp = tempfile::tempdir().unwrap();
        write_app(tmp.path(), "git", r#"{"version":"1.0"}"#, Some(r#"{"bucket":"main"}"#));
        let local = scan(tmp.path(), false);
        assert!(!local[0].global);
        let global = scan(tmp.path(), true);
        assert!(global[0].global);
    }
```

`src-tauri/src/domain/updates.rs`：测试 helper `app()` 的 `InstalledApp` 字面量加 `global: false,`，并在 `mod tests` 内追加：

```rust
    #[test]
    fn passes_global_flag() {
        let mut g = app("git", "1.0", Some("main"), false);
        g.global = true;
        let out = compute(&[g], &[entry("git", "main", "2.0")]);
        assert_eq!(out.len(), 1);
        assert!(out[0].global);
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib`
Expected: 编译失败（`scan` 参数数量不符、`InstalledApp`/`UpdateInfo` 缺 `global` 字段）

- [ ] **Step 3: 实现**

`src-tauri/src/domain/installed.rs`：结构体 `app_dir` 之后加字段；`scan` 签名与字面量改为：

```rust
    pub bin: Vec<String>,
    pub app_dir: String,
    pub global: bool,
}
```

```rust
pub fn scan(apps_dir: &Path, global: bool) -> Vec<InstalledApp> {
```

```rust
            app_dir: dir.display().to_string(),
            global,
        });
```

`src-tauri/src/domain/updates.rs`：`UpdateInfo` 加 `pub global: bool`，`compute` 中 `out.push` 加 `global: app.global,`。

`src-tauri/src/domain/root.rs:92` 改为：

```rust
        app_count: if valid { crate::domain::installed::scan(&path.join("apps"), false).len() } else { 0 },
```

`src-tauri/src/commands/mod.rs`：`impl Context` 内 `settings_dir` 之后加 `global_dir`；`impl` 块之后追加 `scan_installed`：

```rust
    pub fn global_dir(&self) -> PathBuf {
        crate::domain::root::global_dir(
            self.env_global.clone(),
            self.config_global_path(),
            &self.program_data,
        )
    }
}

pub fn scan_installed(ctx: &Context, root: &std::path::Path) -> Vec<crate::domain::installed::InstalledApp> {
    let mut apps = crate::domain::installed::scan(&root.join("apps"), false);
    let global = ctx.global_dir();
    if global.join("apps").is_dir() {
        apps.extend(crate::domain::installed::scan(&global.join("apps"), true));
    }
    apps.sort_by(|a, b| {
        a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.global.cmp(&b.global))
    });
    apps
}
```

`src-tauri/src/commands/apps.rs` 全文替换：

```rust
use tauri::State;

use crate::commands::{context, roots::resolve_active_root, scan_installed};
use crate::domain::installed::InstalledApp;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
pub fn list_installed_apps(state: State<'_, AppState>) -> Result<Vec<InstalledApp>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    Ok(scan_installed(&ctx, &root))
}
```

`src-tauri/src/commands/updates.rs` 全文替换：

```rust
use tauri::State;

use crate::commands::roots::resolve_active_root;
use crate::commands::search::load_index;
use crate::commands::{context, scan_installed};
use crate::domain::updates::{self, UpdateInfo};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn list_updates(state: State<'_, AppState>) -> Result<Vec<UpdateInfo>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = load_index(&state, &root).await?;
    let apps = scan_installed(&ctx, &root);
    Ok(updates::compute(&apps, &entries))
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 106 passed（104 + 2）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/installed.rs src-tauri/src/domain/updates.rs src-tauri/src/domain/root.rs src-tauri/src/commands/mod.rs src-tauri/src/commands/apps.rs src-tauri/src/commands/updates.rs
git commit -m "feat: 后端全局应用扫描与更新"
```

---

### Task 2: profiles 领域基础（类型/校验/存储）

**Files:**
- Modify: `src-tauri/src/paths.rs`
- Create: `src-tauri/src/domain/profiles.rs`
- Modify: `src-tauri/src/domain/mod.rs`

**Interfaces:**
- Produces: `SETTINGS_WHITELIST`、`BucketRef`、`ProfileApp`、`Profile`、`ProfileSummary`、`validate_profile_name`、`exists`、`list`、`load`、`save`、`delete`、`rename`、`now_epoch`、`paths::profiles_dir`

- [ ] **Step 1: 写失败测试**

`src-tauri/src/paths.rs` 的 `joins_expected_subpaths` 追加：

```rust
        assert_eq!(profiles_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\scoopui\\profiles"));
```

新建 `src-tauri/src/domain/profiles.rs`：包含 `SETTINGS_WHITELIST`、`BucketRef`、`ProfileApp`、`Profile`（含 `de_buckets` 兼容字符串数组）、`ProfileSummary`、`now_epoch`，以及完整测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Profile {
        Profile {
            version: 1,
            name: "工作".to_string(),
            created: 1758614400,
            buckets: vec![BucketRef {
                name: "main".to_string(),
                source: Some("https://github.com/ScoopInstaller/Main".to_string()),
            }],
            apps: vec![ProfileApp {
                name: "git".to_string(),
                bucket: Some("main".to_string()),
                global: false,
            }],
            settings: serde_json::json!({ "url_proxy": "https://scoop.201704.xyz" })
                .as_object()
                .unwrap()
                .clone(),
        }
    }

    #[test]
    fn validate_accepts_and_rejects() {
        assert!(validate_profile_name("工作").is_ok());
        assert!(validate_profile_name("my profile_2").is_ok());
        for bad in ["", ".", "..", " 工作", "工作 ", "工作.", "a/b", "a\\b", "C:\\x", "a:b", "a*b", "a\nb"] {
            assert!(validate_profile_name(bad).is_err(), "{bad:?} 应被拒绝");
        }
    }

    #[test]
    fn roundtrip_and_string_buckets() {
        let tmp = tempfile::tempdir().unwrap();
        let p = sample();
        save(tmp.path(), &p).unwrap();
        assert_eq!(load(tmp.path(), "工作").unwrap(), p);

        std::fs::write(
            tmp.path().join("混合.json"),
            r#"{"version":1,"name":"混合","buckets":["main",{"name":"extras","source":"https://e"}],"apps":[]}"#,
        )
        .unwrap();
        let mixed = load(tmp.path(), "混合").unwrap();
        assert_eq!(mixed.buckets.len(), 2);
        assert_eq!(mixed.buckets[0].source, None);
        assert_eq!(mixed.buckets[1].source.as_deref(), Some("https://e"));
    }

    #[test]
    fn list_skips_corrupt_and_sorts() {
        let tmp = tempfile::tempdir().unwrap();
        let mut a = sample();
        a.name = "alpha".to_string();
        save(tmp.path(), &a).unwrap();
        let mut b = sample();
        b.name = "Beta".to_string();
        save(tmp.path(), &b).unwrap();
        std::fs::write(tmp.path().join("broken.json"), "{oops").unwrap();
        let list = list(tmp.path());
        assert_eq!(list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["alpha", "Beta"]);
        assert_eq!(list[0].bucket_count, 1);
        assert_eq!(list[0].app_count, 1);
        assert_eq!(list[0].settings_count, 1);
    }

    #[test]
    fn save_load_delete_rename() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), &sample()).unwrap();
        assert!(exists(tmp.path(), "工作"));
        assert!(!exists(tmp.path(), "缺失"));
        assert!(load(tmp.path(), "缺失").is_err());

        let mut other = sample();
        other.name = "其他".to_string();
        save(tmp.path(), &other).unwrap();
        assert!(rename(tmp.path(), "工作", "其他").is_err());

        rename(tmp.path(), "工作", "新工作").unwrap();
        assert!(!exists(tmp.path(), "工作"));
        assert_eq!(load(tmp.path(), "新工作").unwrap().name, "新工作");

        delete(tmp.path(), "新工作").unwrap();
        assert!(delete(tmp.path(), "新工作").is_err());
    }

    #[test]
    fn save_leaves_no_tmp_file() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), &sample()).unwrap();
        assert!(tmp.path().join("工作.json").is_file());
        assert!(!tmp.path().join("工作.json.tmp").exists());
    }
}
```

`src-tauri/src/domain/mod.rs` 注册模块（此时编译红）：

```rust
pub mod profiles;
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib profiles::`
Expected: 编译失败（`validate_profile_name` / `save` / `load` / `list` / `exists` / `rename` / `delete` 未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/paths.rs` 追加：

```rust
pub fn profiles_dir(appdata: &Path) -> PathBuf {
    settings_dir(appdata).join("profiles")
}
```

`src-tauri/src/domain/profiles.rs` 的 `use` 区：

```rust
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;
```

`ProfileSummary` 之后插入实现：

```rust
pub fn validate_profile_name(name: &str) -> Result<(), AppError> {
    let invalid = name.is_empty()
        || name.trim() != name
        || name == "."
        || name == ".."
        || name.ends_with('.')
        || name
            .chars()
            .any(|c| matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control());
    if invalid {
        return Err(AppError::Other(format!("配置集名称非法: {name}")));
    }
    Ok(())
}

fn profile_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

pub fn exists(dir: &Path, name: &str) -> bool {
    profile_path(dir, name).is_file()
}

pub fn list(dir: &Path) -> Vec<ProfileSummary> {
    let mut out: Vec<ProfileSummary> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
        {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(profile) = serde_json::from_str::<Profile>(&text) else {
            continue;
        };
        out.push(ProfileSummary {
            name: stem,
            created: profile.created,
            bucket_count: profile.buckets.len(),
            app_count: profile.apps.len(),
            settings_count: profile.settings.len(),
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn load(dir: &Path, name: &str) -> Result<Profile, AppError> {
    validate_profile_name(name)?;
    let text = std::fs::read_to_string(profile_path(dir, name))
        .map_err(|_| AppError::Other(format!("配置集不存在: {name}")))?;
    serde_json::from_str(&text).map_err(|_| AppError::Other(format!("配置集文件损坏: {name}")))
}

pub fn save(dir: &Path, profile: &Profile) -> Result<(), AppError> {
    validate_profile_name(&profile.name)?;
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(profile)?;
    let tmp = dir.join(format!("{}.json.tmp", profile.name));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, profile_path(dir, &profile.name))?;
    Ok(())
}

pub fn delete(dir: &Path, name: &str) -> Result<(), AppError> {
    validate_profile_name(name)?;
    let path = profile_path(dir, name);
    if !path.is_file() {
        return Err(AppError::Other(format!("配置集不存在: {name}")));
    }
    std::fs::remove_file(path)?;
    Ok(())
}

pub fn rename(dir: &Path, name: &str, new_name: &str) -> Result<(), AppError> {
    validate_profile_name(name)?;
    validate_profile_name(new_name)?;
    let mut profile = load(dir, name)?;
    if exists(dir, new_name) {
        return Err(AppError::Other(format!("配置集已存在: {new_name}")));
    }
    profile.name = new_name.to_string();
    save(dir, &profile)?;
    std::fs::remove_file(profile_path(dir, name))?;
    Ok(())
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 111 passed（106 + 5）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/paths.rs src-tauri/src/domain/profiles.rs src-tauri/src/domain/mod.rs
git commit -m "feat: profiles 领域基础（类型/校验/存储）"
```

---

### Task 3: snapshot 与 parse_import

**Files:**
- Modify: `src-tauri/src/domain/profiles.rs`

**Interfaces:**
- Consumes: Task 1 的 `InstalledApp.global`；Task 2 的类型
- Produces: `snapshot(name: &str, buckets: &[BucketInfo], apps: &[InstalledApp], config: &Value) -> Profile`；`parse_import(text: &str, name: &str) -> Result<Profile, AppError>`；内部 `is_remote_url`、`whitelist_from`、`validate_setting`（Task 4 复用）

- [ ] **Step 1: 写失败测试**

`src-tauri/src/domain/profiles.rs` 的 `mod tests` 顶部追加 helper，并追加 4 个测试：

```rust
    use crate::domain::buckets::BucketInfo;
    use crate::domain::installed::InstalledApp;

    fn installed_app(name: &str, bucket: Option<&str>, global: bool) -> InstalledApp {
        InstalledApp {
            name: name.to_string(),
            version: "1.0".to_string(),
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

    #[test]
    fn snapshot_collects_buckets_apps_and_whitelist() {
        let buckets = vec![BucketInfo {
            name: "main".to_string(),
            source: Some("https://github.com/ScoopInstaller/Main".to_string()),
            is_git: true,
            manifest_count: 3,
        }];
        let apps = vec![
            installed_app("git", Some("main"), false),
            installed_app("7zip", None, true),
        ];
        let config = serde_json::json!({
            "url_proxy": "https://scoop.201704.xyz",
            "proxy": "",
            "aria2-enabled": false,
            "aria2-split": 5,
            "other_key": 1
        });
        let p = snapshot("工作", &buckets, &apps, &config);
        assert_eq!(p.version, 1);
        assert_eq!(p.name, "工作");
        assert!(p.created > 0);
        assert_eq!(p.buckets.len(), 1);
        assert_eq!(p.buckets[0].source.as_deref(), Some("https://github.com/ScoopInstaller/Main"));
        assert_eq!(p.apps.len(), 2);
        assert!(!p.apps[0].global);
        assert!(p.apps[1].global);
        assert_eq!(p.settings.get("url_proxy").and_then(|v| v.as_str()), Some("https://scoop.201704.xyz"));
        assert_eq!(p.settings.get("aria2-enabled").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(p.settings.get("aria2-split").and_then(|v| v.as_u64()), Some(5));
        assert!(!p.settings.contains_key("proxy"));
        assert!(!p.settings.contains_key("other_key"));
    }

    #[test]
    fn parse_import_reads_own_format_and_overrides_name() {
        let text = r#"{"version":1,"name":"旧名","created":1,"buckets":["main"],"apps":[{"name":"git","bucket":"main","global":false}],"settings":{}}"#;
        let p = parse_import(text, "新名").unwrap();
        assert_eq!(p.name, "新名");
        assert_eq!(p.buckets[0].name, "main");
        assert_eq!(p.apps[0].name, "git");
    }

    #[test]
    fn parse_import_converts_scoop_export() {
        let text = r#"{
          "buckets": [
            {"Name":"main","Source":"https://github.com/ScoopInstaller/Main"},
            {"Name":"local","Source":"C:\\mybucket"}
          ],
          "apps": [
            {"Name":"git","Source":"main","Info":"Held package, 64bit"},
            {"Name":"python","Source":"<auto-generated>","Info":"Global install"},
            {"Name":"tool","Source":"https://example.com/tool.json","Info":""}
          ],
          "config": {"url_proxy":"https://scoop.201704.xyz","root_path":"C:\\scoop","aria2-enabled":true}
        }"#;
        let p = parse_import(text, "导入").unwrap();
        assert_eq!(p.buckets.len(), 2);
        assert_eq!(p.buckets[0].source.as_deref(), Some("https://github.com/ScoopInstaller/Main"));
        assert_eq!(p.buckets[1].source, None);
        assert_eq!(p.apps.len(), 3);
        assert_eq!(p.apps[0].bucket.as_deref(), Some("main"));
        assert!(!p.apps[0].global);
        assert!(p.apps[1].global);
        assert_eq!(p.apps[1].bucket, None);
        assert_eq!(p.apps[2].bucket, None);
        assert_eq!(p.settings.get("url_proxy").and_then(|v| v.as_str()), Some("https://scoop.201704.xyz"));
        assert_eq!(p.settings.get("aria2-enabled").and_then(|v| v.as_bool()), Some(true));
        assert!(!p.settings.contains_key("root_path"));
    }

    #[test]
    fn parse_import_rejects_unknown() {
        assert!(parse_import("{not json", "x").is_err());
        assert!(parse_import("{}", "x").is_err());
        assert!(parse_import(r#"{"buckets":[],"apps":[]}"#, "x").is_err());
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib profiles::`
Expected: 编译失败（`snapshot` / `parse_import` 未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/domain/profiles.rs` 的 `use` 区补：

```rust
use crate::domain::buckets::BucketInfo;
use crate::domain::installed::InstalledApp;
```

文件末尾（测试模块之前）追加：

```rust
fn is_remote_url(source: &str) -> bool {
    let s = source.trim().to_ascii_lowercase();
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("git@") || s.starts_with("ssh://")
}

fn validate_setting(key: &str, value: &Value) -> Result<(), AppError> {
    let ok = match key {
        "url_proxy" | "proxy" => value.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false),
        "aria2-enabled" => value.is_boolean(),
        "aria2-split" | "aria2-max-connection-per-server" => value.as_u64().is_some(),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(AppError::Other(format!("settings.{key} 类型不符")))
    }
}

fn whitelist_from(config: &Value) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    let Some(obj) = config.as_object() else {
        return map;
    };
    for key in SETTINGS_WHITELIST {
        let found = obj
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v);
        if let Some(v) = found {
            if validate_setting(key, v).is_ok() {
                map.insert(key.to_string(), v.clone());
            }
        }
    }
    map
}

pub fn snapshot(
    name: &str,
    buckets: &[BucketInfo],
    apps: &[InstalledApp],
    config: &Value,
) -> Profile {
    Profile {
        version: 1,
        name: name.to_string(),
        created: now_epoch(),
        buckets: buckets
            .iter()
            .map(|b| BucketRef {
                name: b.name.clone(),
                source: b.source.clone(),
            })
            .collect(),
        apps: apps
            .iter()
            .map(|a| ProfileApp {
                name: a.name.clone(),
                bucket: a.bucket.clone(),
                global: a.global,
            })
            .collect(),
        settings: whitelist_from(config),
    }
}

#[derive(Deserialize)]
struct ScoopExport {
    #[serde(default)]
    buckets: Vec<ScoopBucket>,
    #[serde(default)]
    apps: Vec<ScoopApp>,
    #[serde(default)]
    config: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
struct ScoopBucket {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Source", default)]
    source: Option<String>,
}

#[derive(Deserialize)]
struct ScoopApp {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Source", default)]
    source: Option<String>,
    #[serde(rename = "Info", default)]
    info: Option<String>,
}

pub fn parse_import(text: &str, name: &str) -> Result<Profile, AppError> {
    let value: Value =
        serde_json::from_str(text).map_err(|_| AppError::Other("无法识别的配置集文件".to_string()))?;
    if let Ok(profile) = serde_json::from_value::<Profile>(value.clone()) {
        let mut p = profile;
        p.name = name.to_string();
        return Ok(p);
    }
    let export: ScoopExport =
        serde_json::from_value(value).map_err(|_| AppError::Other("无法识别的配置集文件".to_string()))?;
    if export.apps.is_empty() && export.buckets.is_empty() {
        return Err(AppError::Other("无法识别的配置集文件".to_string()));
    }
    let buckets = export
        .buckets
        .into_iter()
        .map(|b| BucketRef {
            name: b.name,
            source: b.source.filter(|s| is_remote_url(s)),
        })
        .collect();
    let apps = export
        .apps
        .into_iter()
        .map(|a| {
            let global = a
                .info
                .as_deref()
                .map(|i| i.split(", ").any(|p| p == "Global install"))
                .unwrap_or(false);
            let bucket = a
                .source
                .filter(|s| !s.trim().is_empty() && s != "<auto-generated>" && !s.contains("://"));
            ProfileApp {
                name: a.name,
                bucket,
                global,
            }
        })
        .collect();
    Ok(Profile {
        version: 1,
        name: name.to_string(),
        created: now_epoch(),
        buckets,
        apps,
        settings: whitelist_from(&Value::Object(export.config)),
    })
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 115 passed（111 + 4）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/profiles.rs
git commit -m "feat: profiles 快照与导入解析"
```

---

### Task 4: diff 与 apply_settings

**Files:**
- Modify: `src-tauri/src/domain/profiles.rs`

**Interfaces:**
- Consumes: Task 3 的 `validate_setting`、`whitelist_from`；`configfile::set_key`
- Produces: `SettingDiff`、`ProfileDiff`、`diff(&Profile, &[BucketInfo], &[InstalledApp], &Value) -> ProfileDiff`、`apply_settings(&Path, &Map<String, Value>) -> Result<(), AppError>`

- [ ] **Step 1: 写失败测试**

`src-tauri/src/domain/profiles.rs` 的 `mod tests` 追加：

```rust
    fn bucket(name: &str, source: Option<&str>) -> BucketInfo {
        BucketInfo {
            name: name.to_string(),
            source: source.map(|s| s.to_string()),
            is_git: source.is_some(),
            manifest_count: 0,
        }
    }

    fn profile_with(
        buckets: Vec<BucketRef>,
        apps: Vec<ProfileApp>,
        settings: serde_json::Map<String, Value>,
    ) -> Profile {
        Profile {
            version: 1,
            name: "p".to_string(),
            created: 0,
            buckets,
            apps,
            settings,
        }
    }

    #[test]
    fn diff_reports_missing_and_extra() {
        let profile = profile_with(
            vec![
                BucketRef {
                    name: "main".to_string(),
                    source: Some("https://github.com/ScoopInstaller/Main".to_string()),
                },
                BucketRef {
                    name: "extras".to_string(),
                    source: None,
                },
            ],
            vec![
                ProfileApp {
                    name: "git".to_string(),
                    bucket: Some("main".to_string()),
                    global: false,
                },
                ProfileApp {
                    name: "7zip".to_string(),
                    bucket: Some("main".to_string()),
                    global: true,
                },
            ],
            serde_json::json!({ "url_proxy": "https://scoop.201704.xyz" })
                .as_object()
                .unwrap()
                .clone(),
        );
        let buckets = vec![
            bucket("main", Some("https://github.com/ScoopInstaller/Main.git/")),
            bucket("java", None),
        ];
        let apps = vec![
            installed_app("git", Some("main"), false),
            installed_app("python", Some("main"), false),
            installed_app("7zip", Some("main"), true),
        ];
        let d = diff(&profile, &buckets, &apps, &serde_json::json!({ "url_proxy": "https://old" }));
        assert_eq!(
            d.missing_buckets.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            vec!["extras"]
        );
        assert_eq!(d.extra_buckets, vec!["java"]);
        assert!(d.bucket_source_conflicts.is_empty());
        assert!(d.missing_apps.is_empty());
        assert_eq!(
            d.extra_apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
            vec!["python"]
        );
        assert_eq!(d.settings.len(), 1);
        assert_eq!(d.settings[0].key, "url_proxy");
        assert_eq!(d.settings[0].current.as_ref().and_then(|v| v.as_str()), Some("https://old"));
        assert_eq!(d.settings[0].target.as_str(), Some("https://scoop.201704.xyz"));
    }

    #[test]
    fn diff_detects_source_conflicts_and_missing_global_app() {
        let profile = profile_with(
            vec![BucketRef {
                name: "main".to_string(),
                source: Some("https://github.com/ScoopInstaller/Main".to_string()),
            }],
            vec![ProfileApp {
                name: "7zip".to_string(),
                bucket: None,
                global: true,
            }],
            serde_json::Map::new(),
        );
        let buckets = vec![bucket("main", Some("https://gitee.com/scoop-installer/Main"))];
        let apps = vec![installed_app("7zip", Some("main"), false)];
        let d = diff(&profile, &buckets, &apps, &serde_json::json!({}));
        assert_eq!(d.bucket_source_conflicts, vec!["main"]);
        assert_eq!(d.missing_apps.len(), 1);
        assert!(d.missing_apps[0].global);
        assert_eq!(d.extra_apps.len(), 1);
        assert!(!d.extra_apps[0].global);
        assert!(d.settings.is_empty());
    }

    #[test]
    fn apply_settings_writes_only_whitelisted_present_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"scoop_branch":"master","proxy":"old:1"}"#).unwrap();
        let settings = serde_json::json!({"url_proxy":"https://scoop.201704.xyz","aria2-enabled":false})
            .as_object()
            .unwrap()
            .clone();
        apply_settings(&path, &settings).unwrap();
        let v = crate::domain::configfile::load(&path);
        assert_eq!(v.get("url_proxy").and_then(|x| x.as_str()), Some("https://scoop.201704.xyz"));
        assert_eq!(v.get("aria2-enabled").and_then(|x| x.as_bool()), Some(false));
        assert_eq!(v.get("proxy").and_then(|x| x.as_str()), Some("old:1"));
        assert_eq!(v.get("scoop_branch").and_then(|x| x.as_str()), Some("master"));
    }

    #[test]
    fn apply_settings_rejects_bad_types() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        let settings = serde_json::json!({ "url_proxy": 42 }).as_object().unwrap().clone();
        let err = apply_settings(&path, &settings).unwrap_err().to_string();
        assert!(err.contains("settings.url_proxy 类型不符"), "实际: {err}");
        assert!(!path.exists());
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib profiles::`
Expected: 编译失败（`diff` / `apply_settings` / `ProfileDiff` / `SettingDiff` 未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/domain/profiles.rs` 的 `ProfileSummary` 之后追加类型：

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingDiff {
    pub key: String,
    pub current: Option<Value>,
    pub target: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileDiff {
    pub missing_buckets: Vec<BucketRef>,
    pub extra_buckets: Vec<String>,
    pub bucket_source_conflicts: Vec<String>,
    pub missing_apps: Vec<ProfileApp>,
    pub extra_apps: Vec<ProfileApp>,
    pub settings: Vec<SettingDiff>,
}
```

`use` 区补 `use crate::domain::configfile;`；在 `parse_import` 之后追加：

```rust
fn normalize_source(source: &str) -> String {
    source
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_ascii_lowercase()
}

fn settings_diff(profile: &Profile, config: &Value) -> Vec<SettingDiff> {
    let mut out = Vec::new();
    for key in SETTINGS_WHITELIST {
        let Some(target) = profile.settings.get(key) else {
            continue;
        };
        let current = config
            .as_object()
            .and_then(|o| o.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.clone()));
        if current.as_ref() != Some(target) {
            out.push(SettingDiff {
                key: key.to_string(),
                current,
                target: target.clone(),
            });
        }
    }
    out
}

pub fn diff(
    profile: &Profile,
    buckets: &[BucketInfo],
    apps: &[InstalledApp],
    config: &Value,
) -> ProfileDiff {
    let lower = |s: &str| s.to_lowercase();
    let current_bucket_names: Vec<String> = buckets.iter().map(|b| lower(&b.name)).collect();
    let mut missing_buckets: Vec<BucketRef> = profile
        .buckets
        .iter()
        .filter(|b| !current_bucket_names.contains(&lower(&b.name)))
        .cloned()
        .collect();
    let profile_bucket_names: Vec<String> = profile.buckets.iter().map(|b| lower(&b.name)).collect();
    let mut extra_buckets: Vec<String> = buckets
        .iter()
        .filter(|b| !profile_bucket_names.contains(&lower(&b.name)))
        .map(|b| b.name.clone())
        .collect();
    let mut bucket_source_conflicts: Vec<String> = Vec::new();
    for pb in &profile.buckets {
        let Some(cb) = buckets.iter().find(|b| b.name.eq_ignore_ascii_case(&pb.name)) else {
            continue;
        };
        if let (Some(ps), Some(cs)) = (pb.source.as_deref(), cb.source.as_deref()) {
            if normalize_source(ps) != normalize_source(cs) {
                bucket_source_conflicts.push(pb.name.clone());
            }
        }
    }
    let app_key = |name: &str, global: bool| (lower(name), global);
    let current_app_keys: Vec<(String, bool)> = apps.iter().map(|a| app_key(&a.name, a.global)).collect();
    let mut missing_apps: Vec<ProfileApp> = profile
        .apps
        .iter()
        .filter(|a| !current_app_keys.contains(&app_key(&a.name, a.global)))
        .cloned()
        .collect();
    let profile_app_keys: Vec<(String, bool)> = profile.apps.iter().map(|a| app_key(&a.name, a.global)).collect();
    let mut extra_apps: Vec<ProfileApp> = apps
        .iter()
        .filter(|a| !profile_app_keys.contains(&app_key(&a.name, a.global)))
        .map(|a| ProfileApp {
            name: a.name.clone(),
            bucket: a.bucket.clone(),
            global: a.global,
        })
        .collect();
    let by_name = |a: &ProfileApp, b: &ProfileApp| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.global.cmp(&b.global))
    };
    missing_buckets.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    extra_buckets.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    bucket_source_conflicts.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    missing_apps.sort_by(by_name);
    extra_apps.sort_by(by_name);
    ProfileDiff {
        missing_buckets,
        extra_buckets,
        bucket_source_conflicts,
        missing_apps,
        extra_apps,
        settings: settings_diff(profile, config),
    }
}

pub fn apply_settings(config_path: &Path, settings: &serde_json::Map<String, Value>) -> Result<(), AppError> {
    let mut writes: Vec<(&str, &Value)> = Vec::new();
    for key in SETTINGS_WHITELIST {
        if let Some(value) = settings.get(key) {
            validate_setting(key, value)?;
            writes.push((key, value));
        }
    }
    for (key, value) in writes {
        configfile::set_key(config_path, key, Some(value.clone()))?;
    }
    Ok(())
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 119 passed（115 + 4）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/profiles.rs
git commit -m "feat: profiles 差异计算与设置应用"
```

---

### Task 5: profiles 命令层与注册

**Files:**
- Create: `src-tauri/src/commands/profiles.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 的 `scan_installed`；Task 2-4 的 domain 函数
- Produces: 命令 `list_profiles` / `get_profile` / `save_profile_from_current` / `import_profile` / `export_profile` / `rename_profile` / `delete_profile` / `profile_diff` / `apply_profile_settings`

- [ ] **Step 1: 实现命令层**（薄适配层，逻辑已由 Task 2-4 的 domain 单测覆盖；本任务验证方式为编译 + 全量测试）

新建 `src-tauri/src/commands/profiles.rs`：

```rust
use tauri::State;

use crate::commands::{context, roots::resolve_active_root, scan_installed};
use crate::domain::buckets;
use crate::domain::configfile;
use crate::domain::profiles::{self, Profile, ProfileDiff, ProfileSummary};
use crate::error::AppError;
use crate::paths;
use crate::state::AppState;

#[tauri::command(async)]
pub fn list_profiles() -> Result<Vec<ProfileSummary>, AppError> {
    let ctx = context();
    Ok(profiles::list(&paths::profiles_dir(&ctx.appdata)))
}

#[tauri::command(async)]
pub fn get_profile(name: String) -> Result<Profile, AppError> {
    let ctx = context();
    profiles::load(&paths::profiles_dir(&ctx.appdata), &name)
}

#[tauri::command(async)]
pub fn save_profile_from_current(state: State<'_, AppState>, name: String) -> Result<Profile, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let dir = paths::profiles_dir(&ctx.appdata);
    profiles::validate_profile_name(&name)?;
    if profiles::exists(&dir, &name) {
        return Err(AppError::Other(format!("配置集已存在: {name}")));
    }
    let bucket_list = buckets::scan(&root.join("buckets"));
    let apps = scan_installed(&ctx, &root);
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    let config = configfile::load(&config_path);
    let profile = profiles::snapshot(&name, &bucket_list, &apps, &config);
    profiles::save(&dir, &profile)?;
    Ok(profile)
}

#[tauri::command(async)]
pub fn import_profile(path: String, name: String) -> Result<Profile, AppError> {
    let ctx = context();
    let dir = paths::profiles_dir(&ctx.appdata);
    profiles::validate_profile_name(&name)?;
    if profiles::exists(&dir, &name) {
        return Err(AppError::Other(format!("配置集已存在: {name}")));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Other(format!("读取导入文件失败: {e}")))?;
    let profile = profiles::parse_import(&text, &name)?;
    profiles::save(&dir, &profile)?;
    Ok(profile)
}

#[tauri::command(async)]
pub fn export_profile(name: String, path: String) -> Result<(), AppError> {
    let ctx = context();
    let profile = profiles::load(&paths::profiles_dir(&ctx.appdata), &name)?;
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&profile)?)?;
    Ok(())
}

#[tauri::command(async)]
pub fn rename_profile(name: String, new_name: String) -> Result<(), AppError> {
    let ctx = context();
    profiles::rename(&paths::profiles_dir(&ctx.appdata), &name, &new_name)
}

#[tauri::command(async)]
pub fn delete_profile(name: String) -> Result<(), AppError> {
    let ctx = context();
    profiles::delete(&paths::profiles_dir(&ctx.appdata), &name)
}

#[tauri::command(async)]
pub fn profile_diff(state: State<'_, AppState>, name: String) -> Result<ProfileDiff, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let profile = profiles::load(&paths::profiles_dir(&ctx.appdata), &name)?;
    let bucket_list = buckets::scan(&root.join("buckets"));
    let apps = scan_installed(&ctx, &root);
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    let config = configfile::load(&config_path);
    Ok(profiles::diff(&profile, &bucket_list, &apps, &config))
}

#[tauri::command(async)]
pub fn apply_profile_settings(
    state: State<'_, AppState>,
    settings: serde_json::Map<String, serde_json::Value>,
) -> Result<(), AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    profiles::apply_settings(&config_path, &settings)
}
```

`src-tauri/src/commands/mod.rs` 的 `pub mod` 列表插入（字母序，`ops` 之后）：

```rust
pub mod profiles;
```

`src-tauri/src/lib.rs` 的 `invoke_handler` 中 `commands::mirrors::test_url_proxy` 之后追加（注意前一行的逗号）：

```rust
            commands::profiles::list_profiles,
            commands::profiles::get_profile,
            commands::profiles::save_profile_from_current,
            commands::profiles::import_profile,
            commands::profiles::export_profile,
            commands::profiles::rename_profile,
            commands::profiles::delete_profile,
            commands::profiles::profile_diff,
            commands::profiles::apply_profile_settings
```

- [ ] **Step 2: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 119 passed（无新增测试，编译与既有测试全绿）

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/commands/profiles.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: profiles 命令层与注册"
```

---

### Task 6: 前端 appKey 与 apps store 键控重构

**Files:**
- Modify: `src/api/types.ts`
- Create: `src/utils/appkey.ts`
- Modify: `src/stores/apps.ts`
- Modify: `src/__tests__/stores.spec.ts`

**Interfaces:**
- Produces: `InstalledApp.global: boolean`、`UpdateInfo.global: boolean`、`appKey(app: {name: string; global: boolean}) => string`、`useAppsStore` 的 `selectedKey` 与 `updateFor(app)`

- [ ] **Step 1: 写失败测试**

`src/__tests__/stores.spec.ts` 修改：

1) `appFixture` 末尾加字段：

```ts
  app_dir: "C:\\scoop\\apps\\git",
  global: false,
};
```

2) `"apps.load 选中在列表变化时回退"` 用例中 3 处 `store.selectedName` 改为 `store.selectedKey`（期望值不变：`"git"` / `null` / `"7zip"`）。

3) `"apps.load 拉取可更新列表"` 的 update fixture 加 `global: false`，并把 `store.updateFor("git")` 改为传应用对象：

```ts
    mocks.listUpdates.mockResolvedValue([
      { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false, global: false },
    ]);
```

```ts
    expect(store.updateFor(appFixture)?.current).toBe("2.53.0.2");
```

4) `"apps.load 旧响应不覆盖新响应"` 中 `store.selectedName` 改为 `store.selectedKey`。

5) 追加新用例（放在 `"apps.load 旧响应不覆盖新响应"` 之后）：

```ts
  it("apps.load 全局应用以独立 key 记录更新", async () => {
    const globalGit = { ...appFixture, global: true, app_dir: "C:\\ProgramData\\scoop\\apps\\git" };
    mocks.listInstalledApps.mockResolvedValue([appFixture, globalGit]);
    mocks.listUpdates.mockResolvedValue([
      { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false, global: true },
    ]);
    const store = useAppsStore();
    await store.load();
    expect(store.selectedKey).toBe("git");
    expect(store.updateFor(appFixture)).toBeNull();
    expect(store.updateFor(globalGit)?.latest).toBe("2.55.0.5");
    store.select("git@global");
    expect(store.selected?.global).toBe(true);
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/stores.spec.ts`
Expected: FAIL（`selectedKey` 为 undefined、`updateFor` 收到字符串返回 null）

- [ ] **Step 3: 实现**

`src/api/types.ts`：`InstalledApp` 的 `app_dir` 之后加 `global: boolean;`；`UpdateInfo` 的 `hold` 之后加 `global: boolean;`。

新建 `src/utils/appkey.ts`：

```ts
export function appKey(app: { name: string; global: boolean }): string {
  return app.global ? `${app.name}@global` : app.name;
}
```

`src/stores/apps.ts` 全文替换：

```ts
import { defineStore } from "pinia";
import { listInstalledApps, listUpdates } from "../api/commands";
import type { InstalledApp, UpdateInfo } from "../api/types";
import { appKey } from "../utils/appkey";

export const useAppsStore = defineStore("apps", {
  state: () => ({
    apps: [] as InstalledApp[],
    updates: {} as Record<string, UpdateInfo>,
    selectedKey: null as string | null,
    loading: false,
    error: null as string | null,
    seq: 0,
  }),
  getters: {
    selected(state): InstalledApp | null {
      return state.apps.find((a) => appKey(a) === state.selectedKey) ?? null;
    },
    updateCount(state): number {
      return Object.keys(state.updates).length;
    },
    updateFor(state) {
      return (app: InstalledApp): UpdateInfo | null => state.updates[appKey(app)] ?? null;
    },
  },
  actions: {
    async load() {
      const seq = ++this.seq;
      this.loading = true;
      this.error = null;
      try {
        const apps = await listInstalledApps();
        if (seq !== this.seq) return;
        this.apps = apps;
        try {
          const list = await listUpdates();
          if (seq !== this.seq) return;
          this.updates = Object.fromEntries(list.map((u) => [appKey(u), u]));
        } catch {
          if (seq === this.seq) this.updates = {};
        }
        if (!this.apps.some((a) => appKey(a) === this.selectedKey)) {
          this.selectedKey = this.apps[0] ? appKey(this.apps[0]) : null;
        }
      } catch (e) {
        if (seq !== this.seq) return;
        this.apps = [];
        this.error = String(e);
      } finally {
        if (seq === this.seq) this.loading = false;
      }
    },
    select(key: string) {
      this.selectedKey = key;
    },
  },
});
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/stores.spec.ts`
Expected: PASS（13 + 1 = 14 tests）

- [ ] **Step 5: 提交**

```bash
git add src/api/types.ts src/utils/appkey.ts src/stores/apps.ts src/__tests__/stores.spec.ts
git commit -m "feat(ui): appKey 键控与全局应用更新记录"
```

---

### Task 7: 已安装页全局应用展示与操作

**Files:**
- Modify: `src/components/app/AppList.vue`
- Modify: `src/components/app/AppDetail.vue`
- Modify: `src/pages/InstalledPage.vue:7`
- Modify: `src/__tests__/app-components.spec.ts`
- Modify: `src/__tests__/app-ops.spec.ts`

**Interfaces:**
- Consumes: Task 6 的 `appKey`、`InstalledApp.global`、`updateFor(app)`
- Produces: 列表「全局」徽标与 key 传参；详情操作按 `app.global` 传参

- [ ] **Step 1: 写失败测试**

`src/__tests__/app-components.spec.ts`：

1) `git` fixture 末尾加 `global: false,`。
2) 第 157 行附近的 updates 字面量加 `global: false`。
3) 第 203 行 `apps.updates = { git: {...} }` 的字面量加 `global: false`。
4) `describe("AppList")` 内追加：

```ts
  it("全局应用显示徽标并按 key 选中", async () => {
    const globalGit: InstalledApp = { ...git, global: true };
    const wrapper = mount(AppList, {
      props: {
        apps: [globalGit],
        selected: "git@global",
        updates: {
          "git@global": { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false, global: true },
        },
      },
    });
    expect(wrapper.text()).toContain("全局");
    expect(wrapper.text()).toContain("可更新");
    expect(wrapper.find(".row").classes()).toContain("active");
    await wrapper.find(".row").trigger("click");
    expect(wrapper.emitted("select")?.[0]).toEqual(["git@global"]);
  });
```

`src/__tests__/app-ops.spec.ts`：

1) `git` fixture 末尾加 `global: false,`。
2) `describe("AppDetail 操作")` 内追加：

```ts
  it("全局应用操作传 global=true", async () => {
    mocks.uninstallPackage.mockResolvedValue(undefined);
    const globalGit: InstalledApp = { ...git, global: true };
    const wrapper = mount(AppDetail, { props: { app: globalGit } });
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.uninstallPackage).toHaveBeenCalledWith("git", true, false, expect.any(String));
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/app-components.spec.ts src/__tests__/app-ops.spec.ts`
Expected: FAIL（无「全局」徽标；`uninstallPackage` 收到 `false`）

- [ ] **Step 3: 实现**

`src/components/app/AppList.vue` 全文替换：

```vue
<template>
  <div class="list">
    <div v-if="apps.length === 0" class="empty">没有已安装的应用</div>
    <div
      v-for="app in apps"
      :key="appKey(app)"
      class="row"
      :class="{ active: appKey(app) === selected }"
      @click="$emit('select', appKey(app))"
    >
      <AppIcon :name="app.name" :size="28" :source="app.icon ?? app.homepage" />
      <div class="info">
        <div class="name">
          {{ app.name }}
          <span v-if="app.global" class="global">全局</span>
          <span v-if="app.hold" class="hold">锁定</span>
          <span v-if="updates[appKey(app)]" class="update">可更新</span>
        </div>
        <div class="meta">
          {{ app.version }}
          <span v-if="app.bucket"> · {{ app.bucket }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "./AppIcon.vue";
import type { InstalledApp, UpdateInfo } from "../../api/types";
import { appKey } from "../../utils/appkey";

defineProps<{ apps: InstalledApp[]; selected: string | null; updates: Record<string, UpdateInfo> }>();
defineEmits<{ select: [name: string] }>();
</script>

<style scoped>
.list {
  overflow: auto;
  height: 100%;
  border-right: 1px solid #efeff5;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
}
.row:hover {
  background: #f5f7fa;
}
.row.active {
  background: #eaf3ff;
}
.name {
  font-size: 14px;
}
.meta {
  font-size: 12px;
  color: #888;
}
.global {
  margin-left: 6px;
  font-size: 11px;
  color: #18a058;
  border: 1px solid #18a058;
  border-radius: 3px;
  padding: 0 4px;
}
.hold {
  margin-left: 6px;
  font-size: 11px;
  color: #f0a020;
  border: 1px solid #f0a020;
  border-radius: 3px;
  padding: 0 4px;
}
.update {
  margin-left: 6px;
  font-size: 11px;
  color: #2080f0;
  border: 1px solid #2080f0;
  border-radius: 3px;
  padding: 0 4px;
}
.empty {
  padding: 24px;
  color: #999;
  text-align: center;
}
</style>
```

`src/components/app/AppDetail.vue` 修改 5 处：

标题区（第 8-11 行）加全局标签：

```vue
          <div class="title">
            {{ app.name }}
            <n-tag v-if="app.global" size="small">全局</n-tag>
            <n-tag v-if="app.hold" size="small" type="warning">锁定</n-tag>
          </div>
```

`update` computed：

```ts
const update = computed(() => (props.app ? apps.updateFor(props.app) : null));
```

三个操作函数中的 `global` 参数改为 `props.app.global`：

```ts
  const record = await runTask(`更新 ${name}`, (taskId) => updatePackage(name, false, props.app!.global, taskId));
```

```ts
  const record = await runTask(`卸载 ${name}`, (taskId) => uninstallPackage(name, props.app!.global, false, taskId));
```

```ts
  const record = await runTask(`${next ? "锁定" : "解除锁定"} ${name}`, (taskId) =>
    setHold(name, next, props.app!.global, taskId),
  );
```

`src/pages/InstalledPage.vue:7` 改为：

```vue
        :selected="apps.selectedKey"
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/app-components.spec.ts src/__tests__/app-ops.spec.ts`
Expected: PASS（新增 2 个用例 + 既有用例全绿）

- [ ] **Step 5: 提交**

```bash
git add src/components/app/AppList.vue src/components/app/AppDetail.vue src/pages/InstalledPage.vue src/__tests__/app-components.spec.ts src/__tests__/app-ops.spec.ts
git commit -m "feat(ui): 已安装页全局应用展示与操作"
```

---

### Task 8: profiles API 与 store

**Files:**
- Modify: `src/api/types.ts`
- Modify: `src/api/commands.ts`
- Create: `src/stores/profiles.ts`
- Create: `src/__tests__/profiles.spec.ts`
- Modify: `src/__tests__/commands.spec.ts`

**Interfaces:**
- Consumes: Task 6 的 apps store（应用完成后刷新）
- Produces: `BucketRef` / `ProfileApp` / `Profile` / `ProfileSummary` / `SettingDiff` / `ProfileDiff` 类型；9 个命令封装；`useProfilesStore`（`load`/`select`/`refreshDiff`/`saveFromCurrent`/`importFrom`/`exportTo`/`rename`/`remove`/`apply`）

- [ ] **Step 1: 写失败测试**

新建 `src/__tests__/profiles.spec.ts`：

```ts
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listProfiles: vi.fn(),
  getProfile: vi.fn(),
  profileDiff: vi.fn(),
  saveProfileFromCurrent: vi.fn(),
  importProfile: vi.fn(),
  exportProfile: vi.fn(),
  renameProfile: vi.fn(),
  deleteProfile: vi.fn(),
  applyProfileSettings: vi.fn(),
  bucketAdd: vi.fn(),
  installPackage: vi.fn(),
  uninstallPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { useLogsStore } from "../stores/logs";
import { useProfilesStore } from "../stores/profiles";
import type { Profile, ProfileDiff } from "../api/types";

const profileFixture: Profile = {
  version: 1,
  name: "工作",
  created: 1758614400,
  buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  apps: [{ name: "7zip", bucket: "main", global: false }],
  settings: { url_proxy: "https://scoop.201704.xyz" },
};

const emptyDiff: ProfileDiff = {
  missing_buckets: [],
  extra_buckets: [],
  bucket_source_conflicts: [],
  missing_apps: [],
  extra_apps: [],
  settings: [],
};

const diffFixture: ProfileDiff = {
  missing_buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  extra_buckets: [],
  bucket_source_conflicts: [],
  missing_apps: [{ name: "7zip", bucket: "main", global: false }],
  extra_apps: [],
  settings: [{ key: "url_proxy", current: null, target: "https://scoop.201704.xyz" }],
};

describe("profiles store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("load 拉取列表并加载首个详情与差异", async () => {
    mocks.listProfiles.mockResolvedValue([
      { name: "工作", created: 1758614400, bucket_count: 1, app_count: 1, settings_count: 1 },
    ]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(emptyDiff);
    const store = useProfilesStore();
    await store.load();
    expect(store.selectedName).toBe("工作");
    expect(store.detail?.name).toBe("工作");
    expect(store.hasDiff).toBe(false);
    expect(store.loading).toBe(false);
  });

  it("load 失败时 error 可读且列表为空", async () => {
    mocks.listProfiles.mockRejectedValue("读取配置集目录失败");
    const store = useProfilesStore();
    await store.load();
    expect(store.list).toEqual([]);
    expect(store.error).toContain("读取配置集目录失败");
  });

  it("apply 依次执行 settings→桶→应用", async () => {
    mocks.profileDiff.mockResolvedValue(diffFixture);
    mocks.applyProfileSettings.mockResolvedValue(undefined);
    mocks.bucketAdd.mockResolvedValue(undefined);
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useProfilesStore();
    store.selectedName = "工作";
    store.detail = profileFixture;
    const running = store.apply(false);
    await flushPromises();
    expect(mocks.applyProfileSettings).toHaveBeenCalledWith(profileFixture.settings);
    expect(mocks.bucketAdd).toHaveBeenCalledTimes(1);
    const bucketTask = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId: bucketTask, code: 0, success: true });
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("7zip", "main", null, false, expect.any(String));
    const installTask = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId: installTask, code: 0, success: true });
    await flushPromises();
    expect(await running).toBe(true);
    expect(store.steps.map((s) => s.status)).toEqual(["success", "success", "success"]);
    expect(mocks.listInstalledApps).toHaveBeenCalled();
  });

  it("apply 失败即停并标记剩余为待执行", async () => {
    mocks.profileDiff.mockResolvedValue({
      ...emptyDiff,
      missing_apps: [
        { name: "7zip", bucket: "main", global: false },
        { name: "git", bucket: "main", global: false },
      ],
    });
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useProfilesStore();
    store.selectedName = "工作";
    store.detail = profileFixture;
    const running = store.apply(false);
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(await running).toBe(false);
    expect(store.steps[0].status).toBe("failed");
    expect(store.steps[1].status).toBe("skipped");
    expect(mocks.installPackage).toHaveBeenCalledTimes(1);
    expect(store.applyError).toContain("退出码 1");
  });

  it("saveFromCurrent 调用命令并刷新", async () => {
    mocks.saveProfileFromCurrent.mockResolvedValue(profileFixture);
    mocks.listProfiles.mockResolvedValue([
      { name: "工作", created: 1, bucket_count: 1, app_count: 1, settings_count: 1 },
    ]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(emptyDiff);
    const store = useProfilesStore();
    await store.saveFromCurrent("工作");
    expect(mocks.saveProfileFromCurrent).toHaveBeenCalledWith("工作");
    expect(store.selectedName).toBe("工作");
  });
});
```

`src/__tests__/commands.spec.ts`：`import` 列表补 `applyProfileSettings, deleteProfile, exportProfile, getProfile, importProfile, listProfiles, profileDiff, renameProfile, saveProfileFromCurrent`，并在 `describe("commands 包装")` 内追加：

```ts
  it("M5a 配置集命令名与参数符合后端契约", async () => {
    await listProfiles();
    expect(invokeMock).toHaveBeenLastCalledWith("list_profiles", undefined);

    await getProfile("工作");
    expect(invokeMock).toHaveBeenLastCalledWith("get_profile", { name: "工作" });

    await saveProfileFromCurrent("工作");
    expect(invokeMock).toHaveBeenLastCalledWith("save_profile_from_current", { name: "工作" });

    await importProfile("C:\\a.json", "工作");
    expect(invokeMock).toHaveBeenLastCalledWith("import_profile", { path: "C:\\a.json", name: "工作" });

    await exportProfile("工作", "C:\\out.json");
    expect(invokeMock).toHaveBeenLastCalledWith("export_profile", { name: "工作", path: "C:\\out.json" });

    await renameProfile("工作", "新工作");
    expect(invokeMock).toHaveBeenLastCalledWith("rename_profile", { name: "工作", newName: "新工作" });

    await deleteProfile("工作");
    expect(invokeMock).toHaveBeenLastCalledWith("delete_profile", { name: "工作" });

    await profileDiff("工作");
    expect(invokeMock).toHaveBeenLastCalledWith("profile_diff", { name: "工作" });

    await applyProfileSettings({ url_proxy: "https://p" });
    expect(invokeMock).toHaveBeenLastCalledWith("apply_profile_settings", {
      settings: { url_proxy: "https://p" },
    });
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/profiles.spec.ts src/__tests__/commands.spec.ts`
Expected: FAIL（`stores/profiles.ts` 不存在，导入失败；新命令封装未定义）

- [ ] **Step 3: 实现**

`src/api/types.ts` 末尾追加：

```ts
export interface BucketRef {
  name: string;
  source: string | null;
}

export interface ProfileApp {
  name: string;
  bucket: string | null;
  global: boolean;
}

export interface Profile {
  version: number;
  name: string;
  created: number;
  buckets: BucketRef[];
  apps: ProfileApp[];
  settings: Record<string, unknown>;
}

export interface ProfileSummary {
  name: string;
  created: number;
  bucket_count: number;
  app_count: number;
  settings_count: number;
}

export interface SettingDiff {
  key: string;
  current: unknown | null;
  target: unknown;
}

export interface ProfileDiff {
  missing_buckets: BucketRef[];
  extra_buckets: string[];
  bucket_source_conflicts: string[];
  missing_apps: ProfileApp[];
  extra_apps: ProfileApp[];
  settings: SettingDiff[];
}
```

`src/api/commands.ts` 末尾追加（`import type` 列表补 `Profile`、`ProfileDiff`、`ProfileSummary`）：

```ts
export const listProfiles = () => invoke<ProfileSummary[]>("list_profiles");
export const getProfile = (name: string) => invoke<Profile>("get_profile", { name });
export const saveProfileFromCurrent = (name: string) => invoke<Profile>("save_profile_from_current", { name });
export const importProfile = (path: string, name: string) => invoke<Profile>("import_profile", { path, name });
export const exportProfile = (name: string, path: string) => invoke<void>("export_profile", { name, path });
export const renameProfile = (name: string, newName: string) => invoke<void>("rename_profile", { name, newName });
export const deleteProfile = (name: string) => invoke<void>("delete_profile", { name });
export const profileDiff = (name: string) => invoke<ProfileDiff>("profile_diff", { name });
export const applyProfileSettings = (settings: Record<string, unknown>) =>
  invoke<void>("apply_profile_settings", { settings });
```

新建 `src/stores/profiles.ts`：

```ts
import { defineStore } from "pinia";
import {
  applyProfileSettings,
  bucketAdd,
  deleteProfile,
  exportProfile,
  getProfile,
  importProfile,
  installPackage,
  listProfiles,
  profileDiff,
  renameProfile,
  saveProfileFromCurrent,
  uninstallPackage,
} from "../api/commands";
import type { Profile, ProfileDiff, ProfileSummary } from "../api/types";
import { runTask } from "../utils/tasks";
import { useAppsStore } from "./apps";
import { useBucketsStore } from "./buckets";
import { useUiStore } from "./ui";

export type ApplyStepStatus = "pending" | "running" | "success" | "failed" | "skipped";

export interface ApplyStep {
  key: string;
  label: string;
  status: ApplyStepStatus;
}

function runTaskFor(step: ApplyStep, diff: ProfileDiff) {
  if (step.key.startsWith("bucket:")) {
    const bucket = diff.missing_buckets.find((b) => step.key === `bucket:${b.name}`)!;
    return runTask(step.label, (taskId) => bucketAdd(bucket.name, bucket.source, taskId));
  }
  if (step.key.startsWith("install:")) {
    const app = diff.missing_apps.find((a) => step.key === `install:${a.name}@${a.global}`)!;
    return runTask(step.label, (taskId) =>
      installPackage(app.name, app.bucket, null, app.global, taskId),
    );
  }
  const app = diff.extra_apps.find((a) => step.key === `uninstall:${a.name}@${a.global}`)!;
  return runTask(step.label, (taskId) => uninstallPackage(app.name, app.global, false, taskId));
}

export const useProfilesStore = defineStore("profiles", {
  state: () => ({
    list: [] as ProfileSummary[],
    selectedName: null as string | null,
    detail: null as Profile | null,
    diff: null as ProfileDiff | null,
    loading: false,
    error: null as string | null,
    applying: false,
    steps: [] as ApplyStep[],
    applyError: null as string | null,
  }),
  getters: {
    hasDiff(state): boolean {
      const d = state.diff;
      if (!d) return false;
      return (
        d.missing_buckets.length > 0 ||
        d.missing_apps.length > 0 ||
        d.extra_apps.length > 0 ||
        d.settings.length > 0
      );
    },
  },
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.list = await listProfiles();
        if (!this.list.some((p) => p.name === this.selectedName)) {
          this.selectedName = this.list[0]?.name ?? null;
        }
      } catch (e) {
        this.list = [];
        this.error = String(e);
      }
      this.loading = false;
      if (!this.selectedName) {
        this.detail = null;
        this.diff = null;
        return;
      }
      try {
        await this.loadDetail();
      } catch (e) {
        this.error = String(e);
      }
    },
    async loadDetail() {
      const name = this.selectedName;
      if (!name) return;
      const [detail, diff] = await Promise.all([getProfile(name), profileDiff(name)]);
      if (this.selectedName !== name) return;
      this.detail = detail;
      this.diff = diff;
    },
    async select(name: string) {
      this.selectedName = name;
      this.error = null;
      try {
        await this.loadDetail();
      } catch (e) {
        this.error = String(e);
      }
    },
    async refreshDiff() {
      const name = this.selectedName;
      if (!name) return;
      this.diff = await profileDiff(name);
    },
    async saveFromCurrent(name: string) {
      await saveProfileFromCurrent(name);
      this.selectedName = name;
      await this.load();
    },
    async importFrom(path: string, name: string) {
      await importProfile(path, name);
      this.selectedName = name;
      await this.load();
    },
    async exportTo(path: string) {
      if (!this.selectedName) return;
      await exportProfile(this.selectedName, path);
    },
    async rename(newName: string) {
      if (!this.selectedName) return;
      await renameProfile(this.selectedName, newName);
      this.selectedName = newName;
      await this.load();
    },
    async remove() {
      if (!this.selectedName) return;
      await deleteProfile(this.selectedName);
      this.selectedName = null;
      await this.load();
    },
    async apply(uninstallExtra: boolean): Promise<boolean> {
      const name = this.selectedName;
      const profile = this.detail;
      if (!name || !profile) return false;
      const diff = await profileDiff(name);
      if (this.selectedName !== name) return false;
      this.diff = diff;
      const steps: ApplyStep[] = [];
      if (diff.settings.length > 0) {
        steps.push({ key: "settings", label: "写入下载加速设置", status: "pending" });
      }
      for (const b of diff.missing_buckets) {
        steps.push({ key: `bucket:${b.name}`, label: `添加桶 ${b.name}`, status: "pending" });
      }
      for (const a of diff.missing_apps) {
        steps.push({
          key: `install:${a.name}@${a.global}`,
          label: `安装 ${a.name}${a.global ? "（全局）" : ""}`,
          status: "pending",
        });
      }
      if (uninstallExtra) {
        for (const a of diff.extra_apps) {
          steps.push({
            key: `uninstall:${a.name}@${a.global}`,
            label: `卸载 ${a.name}${a.global ? "（全局）" : ""}`,
            status: "pending",
          });
        }
      }
      this.steps = steps;
      this.applyError = null;
      this.applying = true;
      useUiStore().logDrawerOpen = true;
      let failed = false;
      for (const step of this.steps) {
        if (failed) {
          step.status = "skipped";
          continue;
        }
        step.status = "running";
        if (step.key === "settings") {
          try {
            await applyProfileSettings(profile.settings);
            step.status = "success";
          } catch (e) {
            step.status = "failed";
            this.applyError = String(e);
            failed = true;
          }
          continue;
        }
        const record = await runTaskFor(step, diff);
        if (record.success) {
          step.status = "success";
        } else {
          step.status = "failed";
          this.applyError = `步骤失败（退出码 ${record.code ?? "未知"}）`;
          failed = true;
        }
      }
      this.applying = false;
      if (!failed) {
        await Promise.all([useAppsStore().load(), useBucketsStore().load()]);
      }
      return !failed;
    },
  },
});
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/profiles.spec.ts src/__tests__/commands.spec.ts`
Expected: PASS（profiles 5 tests + commands 既有 6 + 1 = 7 tests）

- [ ] **Step 5: 提交**

```bash
git add src/api/types.ts src/api/commands.ts src/stores/profiles.ts src/__tests__/profiles.spec.ts src/__tests__/commands.spec.ts
git commit -m "feat(ui): profiles API 与 store"
```

---

### Task 9: 配置集页面与路由

**Files:**
- Create: `src/pages/ProfilesPage.vue`
- Modify: `src/components/layout/AppShell.vue`
- Create: `src/__tests__/profiles-page.spec.ts`
- Modify: `src/__tests__/appshell.spec.ts`

**Interfaces:**
- Consumes: Task 8 的 `useProfilesStore`；`@tauri-apps/plugin-dialog` 的 `open`/`save`
- Produces: `profiles` 页路由；另存/导入/导出/重命名/删除/应用交互

- [ ] **Step 1: 写失败测试**

新建 `src/__tests__/profiles-page.spec.ts`：

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const dialogOpen = vi.hoisted(() => vi.fn());
const dialogSave = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  listProfiles: vi.fn(),
  getProfile: vi.fn(),
  profileDiff: vi.fn(),
  saveProfileFromCurrent: vi.fn(),
  importProfile: vi.fn(),
  exportProfile: vi.fn(),
  renameProfile: vi.fn(),
  deleteProfile: vi.fn(),
  applyProfileSettings: vi.fn(),
  bucketAdd: vi.fn(),
  installPackage: vi.fn(),
  uninstallPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: dialogOpen, save: dialogSave }));
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

import ProfilesPage from "../pages/ProfilesPage.vue";
import { useLogsStore } from "../stores/logs";
import { useProfilesStore } from "../stores/profiles";
import type { Profile, ProfileDiff } from "../api/types";

const summary = { name: "工作", created: 1758614400, bucket_count: 1, app_count: 1, settings_count: 1 };

const profileFixture: Profile = {
  version: 1,
  name: "工作",
  created: 1758614400,
  buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  apps: [{ name: "7zip", bucket: "main", global: false }],
  settings: { url_proxy: "https://scoop.201704.xyz" },
};

const diffFixture: ProfileDiff = {
  missing_buckets: [{ name: "extras", source: "https://gitee.com/scoop-installer/Extras" }],
  extra_buckets: [],
  bucket_source_conflicts: [],
  missing_apps: [{ name: "7zip", bucket: "main", global: false }],
  extra_apps: [],
  settings: [{ key: "url_proxy", current: null, target: "https://scoop.201704.xyz" }],
};

async function mountLoaded() {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useProfilesStore().load();
  const wrapper = mount(ProfilesPage, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}

function bodyButton(text: string): HTMLButtonElement {
  const found = Array.from(document.body.querySelectorAll("button")).find((b) =>
    b.textContent?.includes(text),
  );
  expect(found).toBeTruthy();
  return found as HTMLButtonElement;
}

describe("ProfilesPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listBuckets.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("渲染列表、详情与差异", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(diffFixture);
    const wrapper = await mountLoaded();
    expect(wrapper.text()).toContain("工作");
    expect(wrapper.text()).toContain("缺少桶");
    expect(wrapper.text()).toContain("extras");
    expect(wrapper.text()).toContain("缺少应用");
    expect(wrapper.text()).toContain("7zip");
    expect(wrapper.text()).toContain("url_proxy");
  });

  it("导入时先选文件再确认名称", async () => {
    mocks.listProfiles.mockResolvedValue([]);
    dialogOpen.mockResolvedValue("C:\\tmp\\scoopfile.json");
    mocks.importProfile.mockResolvedValue(profileFixture);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(diffFixture);
    const wrapper = await mountLoaded();
    const importBtn = wrapper.findAll("button").find((b) => b.text().includes("导入"));
    await importBtn!.trigger("click");
    await flushPromises();
    const input = document.body.querySelector("input") as HTMLInputElement;
    expect(input.value).toBe("scoopfile");
    bodyButton("确认导入").click();
    await flushPromises();
    expect(mocks.importProfile).toHaveBeenCalledWith("C:\\tmp\\scoopfile.json", "scoopfile");
  });

  it("导出调用保存对话框与命令", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue(diffFixture);
    dialogSave.mockResolvedValue("C:\\out\\工作.json");
    mocks.exportProfile.mockResolvedValue(undefined);
    const wrapper = await mountLoaded();
    const exportBtn = wrapper.findAll("button").find((b) => b.text().includes("导出"));
    await exportBtn!.trigger("click");
    await flushPromises();
    expect(mocks.exportProfile).toHaveBeenCalledWith("工作", "C:\\out\\工作.json");
  });

  it("应用失败时提示错误并保留步骤状态", async () => {
    mocks.listProfiles.mockResolvedValue([summary]);
    mocks.getProfile.mockResolvedValue(profileFixture);
    mocks.profileDiff.mockResolvedValue({
      ...diffFixture,
      missing_buckets: [],
      settings: [],
      missing_apps: [{ name: "7zip", bucket: "main", global: false }],
    });
    mocks.installPackage.mockResolvedValue(undefined);
    const wrapper = await mountLoaded();
    const applyBtn = wrapper.findAll("button").find((b) => b.text() === "应用");
    await applyBtn!.trigger("click");
    await flushPromises();
    bodyButton("开始执行").click();
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(document.body.textContent).toContain("失败");
  });
});
```

`src/__tests__/appshell.spec.ts`：`stubs` 对象加 `ProfilesPage: true,`，并在 `describe("AppShell 启动时序")` 内追加：

```ts
  it("配置集页渲染 ProfilesPage 而非占位", () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useUiStore().setPage("profiles");
    const wrapper = mount(AppShell, { global: { plugins: [pinia], stubs } });
    expect(wrapper.find("profiles-page-stub").exists()).toBe(true);
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/profiles-page.spec.ts src/__tests__/appshell.spec.ts`
Expected: FAIL（`ProfilesPage.vue` 不存在；AppShell 仍渲染占位）

- [ ] **Step 3: 实现**

新建 `src/pages/ProfilesPage.vue`：

```vue
<template>
  <div class="page">
    <div class="side">
      <div class="side-head">
        <n-button size="small" @click="showSave = true">另存当前状态</n-button>
        <n-button size="small" @click="doImport">导入</n-button>
      </div>
      <n-alert v-if="profiles.error" type="error" size="small" class="alert">{{ profiles.error }}</n-alert>
      <div class="list">
        <div v-if="profiles.list.length === 0" class="empty">从当前状态另存或导入配置集</div>
        <div
          v-for="p in profiles.list"
          :key="p.name"
          class="row"
          :class="{ active: p.name === profiles.selectedName }"
          @click="profiles.select(p.name)"
        >
          <div class="name">{{ p.name }}</div>
          <div class="meta">应用 {{ p.app_count }} · 桶 {{ p.bucket_count }}</div>
          <div class="meta">{{ formatTime(p.created) }}</div>
        </div>
      </div>
    </div>

    <div class="detail">
      <div v-if="!profiles.detail" class="empty">选择一个配置集查看详情</div>
      <template v-else>
        <div class="head">
          <div>
            <div class="title">{{ profiles.detail.name }}</div>
            <div class="sub">创建于 {{ formatTime(profiles.detail.created) }}</div>
          </div>
          <div class="actions">
            <n-button size="small" :disabled="busy" @click="openApply">应用</n-button>
            <n-button size="small" :disabled="busy" @click="doExport">导出</n-button>
            <n-button size="small" :disabled="busy" @click="openRename">重命名</n-button>
            <n-popconfirm
              :positive-button-props="{ disabled: busy, type: 'error' }"
              @positive-click="doDelete"
            >
              <template #trigger>
                <n-button size="small" type="error" :disabled="busy">删除</n-button>
              </template>
              确定删除配置集 {{ profiles.detail.name }}？
            </n-popconfirm>
          </div>
        </div>

        <div class="section">
          <div class="section-title">与当前状态差异</div>
          <div v-if="!profiles.hasDiff" class="hint">与当前状态一致</div>
          <template v-else>
            <div v-if="profiles.diff!.missing_buckets.length" class="diff-line">
              缺少桶：{{ profiles.diff!.missing_buckets.map((b) => b.name).join("、") }}
            </div>
            <div v-if="profiles.diff!.missing_apps.length" class="diff-line">
              缺少应用：{{ profiles.diff!.missing_apps.map(appLabel).join("、") }}
            </div>
            <div v-if="profiles.diff!.extra_apps.length" class="diff-line">
              多余应用（默认不卸载）：{{ profiles.diff!.extra_apps.map(appLabel).join("、") }}
            </div>
            <div v-if="profiles.diff!.extra_buckets.length" class="diff-line">
              多余桶（不处理）：{{ profiles.diff!.extra_buckets.join("、") }}
            </div>
            <div v-if="profiles.diff!.bucket_source_conflicts.length" class="diff-line">
              源冲突（不处理）：{{ profiles.diff!.bucket_source_conflicts.join("、") }}
            </div>
            <div v-if="profiles.diff!.settings.length" class="diff-line">
              设置变化：{{ profiles.diff!.settings.map((s) => s.key).join("、") }}
            </div>
          </template>
        </div>

        <div class="section">
          <div class="section-title">桶（{{ profiles.detail.buckets.length }}）</div>
          <div v-if="profiles.detail.buckets.length === 0" class="hint">无</div>
          <div v-for="b in profiles.detail.buckets" :key="b.name" class="line">
            <span class="mono">{{ b.name }}</span>
            <span class="dim">{{ b.source ?? "（无源）" }}</span>
          </div>
        </div>

        <div class="section">
          <div class="section-title">应用（{{ profiles.detail.apps.length }}）</div>
          <div v-if="profiles.detail.apps.length === 0" class="hint">无</div>
          <div v-for="a in profiles.detail.apps" :key="`${a.name}@${a.global}`" class="line">
            <span class="mono">{{ a.name }}</span>
            <span v-if="a.global" class="badge">全局</span>
            <span class="dim">{{ a.bucket ?? "（未知桶）" }}</span>
          </div>
        </div>

        <div class="section">
          <div class="section-title">下载加速设置</div>
          <div v-if="settingsEntries.length === 0" class="hint">无下载加速设置</div>
          <div v-for="[k, v] in settingsEntries" :key="k" class="line">
            <span class="mono">{{ k }}</span>
            <span class="dim">{{ formatValue(v) }}</span>
          </div>
        </div>
      </template>
    </div>

    <n-modal v-model:show="showSave" preset="card" title="另存当前状态" class="modal-sm">
      <n-input v-model:value="newName" placeholder="配置集名称" />
      <template #footer>
        <n-button size="small" @click="showSave = false">取消</n-button>
        <n-button size="small" type="primary" :disabled="!newName.trim()" @click="doSave">保存</n-button>
      </template>
    </n-modal>

    <n-modal v-model:show="showRename" preset="card" title="重命名配置集" class="modal-sm">
      <n-input v-model:value="renameName" placeholder="新名称" />
      <template #footer>
        <n-button size="small" @click="showRename = false">取消</n-button>
        <n-button size="small" type="primary" :disabled="!renameName.trim()" @click="doRename">确定</n-button>
      </template>
    </n-modal>

    <n-modal v-model:show="showImport" preset="card" title="导入配置集" class="modal-sm">
      <div class="hint">{{ importPath }}</div>
      <n-input v-model:value="importName" placeholder="配置集名称" />
      <template #footer>
        <n-button size="small" @click="showImport = false">取消</n-button>
        <n-button size="small" type="primary" :disabled="!importName.trim()" @click="doConfirmImport">
          确认导入
        </n-button>
      </template>
    </n-modal>

    <n-modal v-model:show="showApply" preset="card" title="应用配置集" class="modal">
      <div v-if="profiles.diff" class="hint">
        缺少桶 {{ profiles.diff.missing_buckets.length }} · 缺少应用 {{ profiles.diff.missing_apps.length }} ·
        多余应用 {{ profiles.diff.extra_apps.length }} · 设置变化 {{ profiles.diff.settings.length }}
      </div>
      <div v-for="s in profiles.steps" :key="s.key" class="step" :class="s.status">
        <span class="status">{{ statusText(s.status) }}</span>
        <span>{{ s.label }}</span>
      </div>
      <n-checkbox v-model:checked="uninstallExtra" :disabled="profiles.applying">同时卸载多余应用</n-checkbox>
      <n-alert v-if="profiles.applyError" type="error" size="small" class="alert">{{ profiles.applyError }}</n-alert>
      <template #footer>
        <n-button size="small" :disabled="profiles.applying" @click="showApply = false">关闭</n-button>
        <n-button size="small" type="primary" :loading="profiles.applying" @click="doApply">
          {{ profiles.applyError ? "重新计算并重试" : "开始执行" }}
        </n-button>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NAlert, NButton, NCheckbox, NInput, NModal, NPopconfirm, useMessage } from "naive-ui";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ProfileApp } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useProfilesStore } from "../stores/profiles";
import type { ApplyStepStatus } from "../stores/profiles";

const profiles = useProfilesStore();
const logs = useLogsStore();
const message = useMessage();
const busy = computed(() => logs.busy);

const showSave = ref(false);
const newName = ref("");
const showRename = ref(false);
const renameName = ref("");
const showImport = ref(false);
const importPath = ref("");
const importName = ref("");
const showApply = ref(false);
const uninstallExtra = ref(false);

const settingsEntries = computed(() => Object.entries(profiles.detail?.settings ?? {}));

function formatTime(sec: number): string {
  if (!sec) return "-";
  return new Date(sec * 1000).toLocaleString();
}

function formatValue(v: unknown): string {
  return typeof v === "string" ? v : JSON.stringify(v);
}

function appLabel(a: ProfileApp): string {
  return a.global ? `${a.name}（全局）` : a.name;
}

function statusText(s: ApplyStepStatus): string {
  const map: Record<ApplyStepStatus, string> = {
    pending: "待执行",
    running: "执行中",
    success: "成功",
    failed: "失败",
    skipped: "待执行",
  };
  return map[s];
}

async function doSave() {
  const name = newName.value.trim();
  if (!name) return;
  try {
    await profiles.saveFromCurrent(name);
    showSave.value = false;
    newName.value = "";
    message.success(`已保存配置集 ${name}`);
  } catch (e) {
    message.error(String(e));
  }
}

function openRename() {
  renameName.value = profiles.selectedName ?? "";
  showRename.value = true;
}

async function doRename() {
  const name = renameName.value.trim();
  if (!name) return;
  try {
    await profiles.rename(name);
    showRename.value = false;
    message.success("已重命名");
  } catch (e) {
    message.error(String(e));
  }
}

async function doDelete() {
  try {
    await profiles.remove();
    message.success("已删除");
  } catch (e) {
    message.error(String(e));
  }
}

async function doImport() {
  const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
  if (typeof path !== "string") return;
  importPath.value = path;
  importName.value = path.split(/[\\/]/).pop()?.replace(/\.json$/i, "") ?? "";
  showImport.value = true;
}

async function doConfirmImport() {
  const name = importName.value.trim();
  if (!name) return;
  try {
    await profiles.importFrom(importPath.value, name);
    showImport.value = false;
    message.success(`已导入配置集 ${name}`);
  } catch (e) {
    message.error(String(e));
  }
}

async function doExport() {
  if (!profiles.detail) return;
  const path = await save({
    defaultPath: `${profiles.detail.name}.json`,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path) return;
  try {
    await profiles.exportTo(path);
    message.success("已导出");
  } catch (e) {
    message.error(String(e));
  }
}

async function openApply() {
  uninstallExtra.value = false;
  showApply.value = true;
  try {
    await profiles.refreshDiff();
  } catch (e) {
    message.error(String(e));
  }
}

async function doApply() {
  const ok = await profiles.apply(uninstallExtra.value);
  if (ok) {
    message.success("配置集应用完成");
    showApply.value = false;
  } else {
    message.error(profiles.applyError ?? "应用失败，详见任务日志");
  }
}
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.side {
  width: 300px;
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
.step {
  display: flex;
  gap: 8px;
  font-size: 13px;
  margin-bottom: 4px;
}
.step .status {
  width: 48px;
  flex-shrink: 0;
  color: #888;
}
.step.success .status {
  color: #18a058;
}
.step.failed .status {
  color: #d03050;
}
.step.running .status {
  color: #2080f0;
}
.empty {
  padding: 24px;
  color: #999;
  text-align: center;
}
.modal {
  width: 460px;
}
.modal-sm {
  width: 360px;
}
</style>
```

`src/components/layout/AppShell.vue`：

1) 模板在 `MaintenancePage` 之后插入：

```vue
        <ProfilesPage v-else-if="ui.activePage === 'profiles'" />
```

2) `script setup` 导入：

```ts
import ProfilesPage from "../../pages/ProfilesPage.vue";
```

3) `placeholders` 删除 `profiles` 行，只保留：

```ts
const placeholders: Record<string, string> = {
  migrate: "环境迁移（M6：注册表扫描与接管）",
  backup: "备份（M5：快照与恢复）",
};
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/profiles-page.spec.ts src/__tests__/appshell.spec.ts`
Expected: PASS（新增 4 + 1 个用例 + 既有用例全绿）

- [ ] **Step 5: 提交**

```bash
git add src/pages/ProfilesPage.vue src/components/layout/AppShell.vue src/__tests__/profiles-page.spec.ts src/__tests__/appshell.spec.ts
git commit -m "feat(ui): 配置集页面与路由"
```

---

### Task 10: 全量验证与手工验收

**Files:** 无新增；本任务只做验证与收尾

- [ ] **Step 1: 后端全量测试**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 119 passed、0 failed

- [ ] **Step 2: 前端全量测试与构建**

Run: `pnpm test`
Expected: 约 135 passed（121 + stores 1 + app-components 1 + app-ops 1 + profiles 5 + commands 1 + profiles-page 4 + appshell 1）

Run: `pnpm build`
Expected: `vue-tsc` 无错误，vite build 成功

- [ ] **Step 3: 手工验收清单（用户在 Windows 本机执行 `pnpm tauri dev`）**

1. 已安装页：若存在全局应用（`%ProgramData%\scoop\apps` 或 `SCOOP_GLOBAL` 指向目录），列表显示「全局」徽标；更新/卸载/锁定对全局应用使用 `--global`
2. 配置集页：另存当前状态 → 列表出现、详情桶/应用/settings 正确；重复名称报「配置集已存在」
3. 差异：手动删一个桶或装一个应用后，差异区正确显示缺少/多余项与 settings 变化
4. 应用：确认弹窗显示差异摘要 → 开始执行 → 步骤逐条变绿 → 完成后差异变为「与当前状态一致」
5. 失败路径：断网或在安装失败场景应用 → 失败步骤红标、剩余显示「待执行」、日志抽屉有输出；「重新计算并重试」可继续
6. 导入：`scoop export > scoopfile.json`（可加 `-c`）→ 导入 → 全局应用带「全局」标记、settings 只取白名单键
7. 导出：导出到桌面 → 文件为自家格式（含 version/name/created/buckets/apps/settings）
8. 重命名与删除：重命名后选中项跟随；删除需确认且列表刷新

- [ ] **Step 4: 修复与提交（仅当验证发现问题）**

若上述步骤发现问题：修复 → 回到 Step 1 重新验证 → 提交修复（`fix: ...`）。全部通过则无需额外提交。

---

## Self-Review 记录

**规格覆盖（对照 `2026-09-23-scoopui-m5a-profiles-design.md`）：**

| 规格章节 | 任务 |
|---|---|
| §2.1 存储位置与命名 | Task 2（`profiles_dir`、`validate_profile_name`、`exists`） |
| §2.2 Profile JSON 与导入兼容 | Task 2（类型/de_buckets）、Task 3（parse_import） |
| §2.3 settings 白名单与类型 | Task 3（`whitelist_from`/`validate_setting`）、Task 4（`apply_settings`） |
| §3.1 domain 函数 | Task 2-4 |
| §3.2 导入格式识别 | Task 3 |
| §3.3 差异计算 | Task 4 |
| §3.4 命令层 | Task 5 |
| §4 全局应用支持 | Task 1（后端）、Task 6（appKey/store）、Task 7（组件） |
| §5 应用流程 | Task 8（store 编排）、Task 9（弹窗与进度） |
| §6 前端设计 | Task 6-9 |
| §7 错误处理 | Task 2/3/4（错误消息）、Task 8/9（message.error）、Task 10（失败路径验收） |
| §8 测试策略 | 各任务 Step 1/4 + Task 10 |

**占位符扫描：** 无 TBD/TODO；所有步骤含完整代码与精确命令。

**类型一致性：**

- `ProfileDiff` 字段（missing_buckets/extra_buckets/bucket_source_conflicts/missing_apps/extra_apps/settings）在 Task 4 定义，Task 8 前端类型、Task 9 模板、store 测试 fixture 完全一致
- `appKey` 在 Task 6 定义，Task 7 组件与 Task 6 store 一致；`UpdateInfo.global` 与 `appKey(u)` 兼容
- `ApplyStep`/`ApplyStepStatus` 在 Task 8 定义，Task 9 导入同一类型
- `bucketAdd(name, repo, taskId)` / `installPackage(name, bucket, version, global, taskId)` / `uninstallPackage(name, global, purge, taskId)` 参数顺序与 `src/api/commands.ts` 既有签名一致
- 命令参数 camelCase→snake_case：`newName`→`new_name`（Tauri 2 自动转换），与既有 `taskId`→`task_id` 模式一致

**已知风险：**

- Task 7 对 `AppDetail.vue` 使用 `props.app!.global`：三处均在 `if (!props.app) return;` 之后，运行期安全
- Task 9 页面测试依赖 naive-ui 弹窗 teleport 到 `document.body`（`attachTo: document.body` + `afterEach` 清理）
- Task 8 的 `apply` 用 `for (const step of this.steps)` 迭代以保持响应式状态更新；步骤对象必须在赋值 `this.steps` 之后通过 store 状态读写




