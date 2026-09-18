# ScoopUI M3a（后端：应用操作 / 桶管理 / 维护命令）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 后端提供类型化的 scoop 操作命令（安装/卸载/更新/hold/桶增删/维护清理），复用现有任务执行管线（流式事件 + 取消），并加单任务串行守卫；作为 M3b 前端的稳定契约。

**Architecture:** 参数构造放纯函数模块 `src-tauri/src/domain/scoopops.rs`（可单测、不依赖 Tauri/进程）；`commands/runner.rs` 抽出 `spawn_task`（root 解析 → `scoopcmd::build/spawn` → 事件线程）并加 `ensure_idle` 串行守卫；`commands/ops.rs` 薄命令层逐命令调 `scoopops` + `spawn_task`。事件协议（`scoop-log`/`scoop-task-end`）与 `run_scoop`/`cancel_task` 契约不变。

**Tech Stack:** Rust（Tauri 2）、既有 `scoopcmd`/`state`/`AppError`。无新增依赖。

## Global Constraints

- **契约（M3b 依赖，字面一致）**：
  - `install_package(name: String, bucket: Option<String>, version: Option<String>, global: bool, taskId: String) -> Result<(), AppError>`
  - `uninstall_package(name: String, global: bool, purge: bool, taskId: String) -> Result<(), AppError>`
  - `update_package(name: Option<String>, all: bool, global: bool, taskId: String) -> Result<(), AppError>`
  - `set_hold(name: String, hold: bool, global: bool, taskId: String) -> Result<(), AppError>`
  - `bucket_add(name: String, repo: Option<String>, taskId: String) -> Result<(), AppError>`
  - `bucket_remove(name: String, taskId: String) -> Result<(), AppError>`
  - `run_maintenance(op: String, taskId: String) -> Result<(), AppError>`，`op ∈ {cleanup_all, cleanup_global, cache_rm_all, checkup, reset_all}`
  - 所有命令同步受理（立即返回），随后异步发 `scoop-log` / `scoop-task-end`（与 `run_scoop` 一致）
- **串行守卫**：同一时刻仅允许一个任务；忙碌时 reject 文案必须为 `已有任务正在运行，请等待完成或取消后再试`
- scoop 参数语法（本机实测）：
  - `install <[bucket/]app[@version]>` + `-g`
  - `uninstall <app>` + `-g` + `-p`
  - `update`（scoop+桶）/ `update <app>`（+`-g`）/ `update -a`（全部应用）
  - `hold|unhold <app>` + `-g`
  - `bucket add <name> [<repo>]` / `bucket rm <name>`
  - `cleanup -a`（+`-g`）/ `cache rm -a` / `checkup` / `reset -a`
- 代码不加注释；错误面向用户中文；不新增依赖；平台 Windows
- 验证门：`cargo check` 0 error、`cargo test` 全绿（不跑 `--ignored`）
- 初始 BASE 提交：`cde335c`

---

### Task 1: 参数构造模块（domain/scoopops.rs）

**Files:**
- Create: `src-tauri/src/domain/scoopops.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod scoopops;`）

**Interfaces:**
- Consumes: `crate::AppError`
- Produces（Task 3 引用）:
  - `install_args(name: &str, bucket: Option<&str>, version: Option<&str>, global: bool) -> Result<Vec<String>, AppError>`
  - `uninstall_args(name: &str, global: bool, purge: bool) -> Result<Vec<String>, AppError>`
  - `update_args(name: Option<&str>, all: bool, global: bool) -> Result<Vec<String>, AppError>`
  - `hold_args(name: &str, hold: bool, global: bool) -> Result<Vec<String>, AppError>`
  - `bucket_add_args(name: &str, repo: Option<&str>) -> Result<Vec<String>, AppError>`
  - `bucket_remove_args(name: &str) -> Result<Vec<String>, AppError>`
  - `maintenance_args(op: &str) -> Result<Vec<String>, AppError>`
  - 空白/全空格 name → `AppError::Other`（中文）；空白可选参数（bucket/version/repo）按 None 处理

- [ ] **Step 1: 写失败测试**

创建 `src-tauri/src/domain/scoopops.rs`（先只放测试与空实现会编译失败；按 TDD 先写如下测试，实现留到 Step 3）：

```rust
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
```

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test scoopops --lib 2>&1 | grep -E "error|test result"` → 编译失败（函数不存在）——红。

- [ ] **Step 2: 注册模块（保持红）**

`src-tauri/src/domain/mod.rs` 增加 `pub mod scoopops;`（若 `mod.rs` 用 `pub mod xxx;` 列表，保持字母序）。

- [ ] **Step 3: 实现**

在 `scoopops.rs` 顶部（测试模块之前）写入：

```rust
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
    if let Some(name) = optional(name, "应用名")? {
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
```

注意：`update_args(None, false, false)` 测试期望 `["update"]`；`update_args(None, true, false)` 期望 `["update","-a"]`；`update_args(Some("  "), false, false)` 按 `["update"]`（可选参数空白视为 None，与 install 一致）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test scoopops --lib 2>&1 | grep -E "test result|^test "` → `7 passed`。
再跑全量：`cargo test 2>&1 | grep -E "test result"` 全绿；`cargo check` 0 error。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/scoopops.rs src-tauri/src/domain/mod.rs
git commit -m "feat: scoop 操作参数构造（安装/卸载/更新/hold/桶/维护）"
```

---

### Task 2: runner 抽出 spawn_task + 单任务串行守卫

**Files:**
- Modify: `src-tauri/src/commands/runner.rs`

**Interfaces:**
- Consumes: 现有 `context`/`resolve_active_root`/`root::global_dir`/`scoopcmd`
- Produces（Task 3 引用）:
  - `pub(crate) fn ensure_idle(tasks: &HashMap<String, u32>) -> Result<(), AppError>`（空 map → Ok；否则 `AppError::Other("已有任务正在运行，请等待完成或取消后再试")`）
  - `pub(crate) fn spawn_task(app: &AppHandle, state: &AppState, args: Vec<String>, task_id: String) -> Result<(), AppError>`（root 解析 → build → 锁内 `ensure_idle` + spawn + 登记 pid → 事件线程；行为与现 `run_scoop` 相同 + 串行守卫）
  - `run_scoop` 改为 `spawn_task` 的薄包装，签名与事件行为不变

- [ ] **Step 1: 写失败测试（ensure_idle）**

在 `runner.rs` 测试模块追加：

```rust
    #[test]
    fn ensure_idle_rejects_when_task_running() {
        let mut tasks = std::collections::HashMap::new();
        assert!(ensure_idle(&tasks).is_ok());
        tasks.insert("t".to_string(), 1234);
        let err = ensure_idle(&tasks).unwrap_err();
        assert!(err.to_string().contains("已有任务正在运行"));
    }
```

同时在测试模块顶部 `use super::*;` 已含；如缺少 `std::collections::HashMap` 可直接全限定（上面的写法已全限定）。

Run: `cargo test ensure_idle --lib 2>&1 | grep -E "error|test result"` → 红（函数不存在）。

- [ ] **Step 2: 实现抽取与守卫**

`runner.rs` 顶部 `use` 增加 `use std::collections::HashMap;`（若与测试冲突可用全限定），并把 `run_scoop` 改为：

```rust
pub(crate) fn ensure_idle(tasks: &HashMap<String, u32>) -> Result<(), AppError> {
    if tasks.is_empty() {
        Ok(())
    } else {
        Err(AppError::Other("已有任务正在运行，请等待完成或取消后再试".to_string()))
    }
}

pub(crate) fn spawn_task(
    app: &AppHandle,
    state: &AppState,
    args: Vec<String>,
    task_id: String,
) -> Result<(), AppError> {
    let ctx = context();
    let root_path = resolve_active_root(&ctx, state).ok_or(AppError::NoRoot)?;
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    let cmd = scoopcmd::build(&root_path, Some(&global), &args)?;
    let spawned = {
        let mut tasks = state.tasks.lock().unwrap();
        ensure_idle(&tasks)?;
        let spawned = scoopcmd::spawn(&cmd)?;
        tasks.insert(task_id.clone(), spawned.pid());
        spawned
    };

    let app_handle = app.clone();
    let task = task_id.clone();
    std::thread::spawn(move || {
        let emit_handle = app_handle.clone();
        let emit_task = task.clone();
        let result = scoopcmd::stream_child(spawned, move |l| {
            let _ = emit_handle.emit(
                "scoop-log",
                LogPayload {
                    task_id: emit_task.clone(),
                    stream: l.stream.to_string(),
                    line: l.line,
                },
            );
        });
        let (code, success) = match result {
            Ok(c) => (c, c == Some(0)),
            Err(_) => (None, false),
        };
        let _ = app_handle.emit(
            "scoop-task-end",
            EndPayload { task_id: task.clone(), code, success },
        );
        if let Some(state) = app_handle.try_state::<AppState>() {
            state.tasks.lock().unwrap().remove(&task);
        }
    });
    Ok(())
}

#[tauri::command(async)]
pub fn run_scoop(
    app: AppHandle,
    state: State<'_, AppState>,
    args: Vec<String>,
    task_id: String,
) -> Result<(), AppError> {
    spawn_task(&app, &state, args, task_id)
}
```

要点：
- 锁只覆盖 `ensure_idle + spawn + insert`（毫秒级），`scoopcmd::build` 的 `is_file` 检查在锁外
- 事件线程内 `state.tasks.remove` 逻辑原样保留
- `LogPayload`/`EndPayload`/`cancel_task_inner`/`cancel_task` 不变

- [ ] **Step 3: 运行测试确认通过**

Run: `cargo test runner:: --lib 2>&1 | grep -E "test result|^test "` → 6 passed（既有 5 + ensure_idle 1）。
全量 `cargo test` 全绿；`cargo check` 0 error。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands/runner.rs
git commit -m "refactor: 抽出 spawn_task 并加单任务串行守卫"
```

---

### Task 3: 操作命令层（commands/ops.rs）与注册

**Files:**
- Create: `src-tauri/src/commands/ops.rs`
- Modify: `src-tauri/src/commands/mod.rs`（`pub mod ops;`）
- Modify: `src-tauri/src/lib.rs`（注册 7 个命令）

**Interfaces:**
- Consumes: Task 1 `scoopops::*`；Task 2 `runner::spawn_task`
- Produces（M3b 契约，见 Global Constraints）

- [ ] **Step 1: 实现命令层**

创建 `src-tauri/src/commands/ops.rs`：

```rust
use tauri::{AppHandle, State};

use crate::commands::runner::spawn_task;
use crate::domain::scoopops;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
pub fn install_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    bucket: Option<String>,
    version: Option<String>,
    global: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::install_args(&name, bucket.as_deref(), version.as_deref(), global)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn uninstall_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    global: bool,
    purge: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::uninstall_args(&name, global, purge)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn update_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: Option<String>,
    all: bool,
    global: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::update_args(name.as_deref(), all, global)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn set_hold(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    hold: bool,
    global: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::hold_args(&name, hold, global)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn bucket_add(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    repo: Option<String>,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::bucket_add_args(&name, repo.as_deref())?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn bucket_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::bucket_remove_args(&name)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn run_maintenance(
    app: AppHandle,
    state: State<'_, AppState>,
    op: String,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::maintenance_args(&op)?;
    spawn_task(&app, &state, args, task_id)
}
```

`src-tauri/src/commands/mod.rs` 的模块列表加 `pub mod ops;`（保持字母序，位于 `icons` 与 `roots` 之间）。

- [ ] **Step 2: 注册命令**

`src-tauri/src/lib.rs` 的 `generate_handler!` 在 `commands::icons::fetch_icon` 之后追加：

```rust
            commands::ops::install_package,
            commands::ops::uninstall_package,
            commands::ops::update_package,
            commands::ops::set_hold,
            commands::ops::bucket_add,
            commands::ops::bucket_remove,
            commands::ops::run_maintenance
```

- [ ] **Step 3: 验证**

Run:
```bash
cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo check 2>&1 | grep -E "error|warning: unused" ; cargo test 2>&1 | grep -E "test result"
```
Expected: `cargo check` 0 error（允许既有 warning 无）；`cargo test` 全绿（预计 76 条：68 + 7 scoopops + 1 ensure_idle）。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands/ops.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: 安装/卸载/更新/hold/桶/维护操作命令与注册"
```

---

## M3a 验收清单（人工）

- [ ] `cargo check` 无 error
- [ ] `cargo test` 全绿（预计 76 条）
- [ ] 契约稳定：7 个命令名/参数/串行错误文案与 Global Constraints 一致
- [ ] 现 `run_scoop`/`cancel_task` 行为与事件不变（M1b 前端无需改动即可继续工作）

## Self-Review 记录

- 规格覆盖：设计 §4.2（命令执行框架/流式/取消）、§2（任务串行队列）、§5（已安装操作、商店安装、桶管理、维护页所需后端能力）对应到 Task 1-3；M4 的镜像/aria2 配置写入、M5-M6 不在本计划。
- 参数语法以本机 scoop（gitee fork）实际脚本 usage 为准：`install`/`uninstall`/`update`/`hold`/`bucket`/`cache`/`cleanup`/`reset`/`checkup` 已逐条核对。
- 占位符扫描：无 TBD；全部含完整代码。
- 类型一致性：命令名与参数在 Global Constraints 与 ops.rs 逐字一致；`spawn_task` 供 Task 3 使用；`maintenance_args` op 白名单与契约一致。
- 已知取舍：`--quiet`/`--arch`/`--independent` 等安装旗标暂不暴露（YAGNI，M3b 无入口）；维护仅提供全局操作（单应用 clean/reset 留待后续）。
- 风险：`spawn_task` 在持 `tasks` 锁期间 spawn 进程（毫秒级），`cancel_task` 可能短暂阻塞，可接受。
