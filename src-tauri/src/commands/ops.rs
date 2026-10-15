use tauri::{AppHandle, State};

use crate::commands::runner::{spawn_task, spawn_task_scoped};
use crate::commands::{context, roots::resolve_scoop_dir};
use crate::domain::postinstall;
use crate::domain::scoopops;
use crate::error::AppError;
use crate::state::{AppState, TaskScope};

#[tauri::command(async)]
pub fn install_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    bucket: Option<String>,
    version: Option<String>,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::install_args(&name, bucket.as_deref(), version.as_deref())?;
    spawn_task_scoped(&app, &state, args, task_id, TaskScope::App(name), None)
}

#[tauri::command(async)]
pub fn uninstall_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    purge: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::uninstall_args(&name, purge)?;
    let after_success = after_uninstall_cleanup(&state, &name, false);
    spawn_task_scoped(&app, &state, args, task_id, TaskScope::App(name), after_success)
}

/// 强制卸载：先结束该应用目录下的进程，再走正常 `scoop uninstall`。
///
/// 用于「正在运行的软件也可删除」：Scoop 检测到进程在运行会拒绝卸载（且退出码仍为 0），
/// 这里由用户在弹窗确认后先结束进程，让 Scoop 完成 shims/注册表/目录全套清理。
#[tauri::command(async)]
pub fn force_uninstall_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    purge: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::uninstall_args(&name, purge)?;
    if postinstall::safe_app_name(&name) {
        let ctx = context();
        let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
        let dir = root.join("apps").join(&name);
        let procs = app_processes(&dir);
        for p in &procs {
            let _ = crate::domain::scoopcmd::kill_tree(p.pid);
        }
        if !procs.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }
    let after_success = after_uninstall_cleanup(&state, &name, true);
    spawn_task_scoped(&app, &state, args, task_id, TaskScope::App(name), after_success)
}

/// 运行中的应用进程（`Path` 位于应用目录下）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppProcessInfo {
    pub pid: u32,
    pub name: String,
}

#[tauri::command(async)]
pub fn list_app_processes(
    state: State<'_, AppState>,
    name: String,
) -> Result<Vec<AppProcessInfo>, AppError> {
    if !postinstall::safe_app_name(&name) {
        return Err(AppError::Other(format!("应用名称非法: {name}")));
    }
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    Ok(app_processes(&root.join("apps").join(&name)))
}

/// 枚举可执行文件位于 `dir` 下的进程（与 Scoop 自身的运行检查口径一致）。
fn app_processes(dir: &std::path::Path) -> Vec<AppProcessInfo> {
    let pattern = format!("{}\\*", dir.display()).replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; @(Get-Process | Where-Object {{ $_.Path -like '{}' }} | Select-Object Id, ProcessName) | ConvertTo-Json -Compress",
        pattern
    );
    let mut cmd = std::process::Command::new("powershell.exe");
    crate::domain::proc::hide_window(&mut cmd);
    let Ok(out) = cmd
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
    else {
        return Vec::new();
    };
    parse_process_list(&String::from_utf8_lossy(&out.stdout))
}

/// 解析 `ConvertTo-Json` 输出的进程列表（兼容单对象/数组/空输出）。
fn parse_process_list(text: &str) -> Vec<AppProcessInfo> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let items: Vec<&serde_json::Value> = match &value {
        serde_json::Value::Array(list) => list.iter().collect(),
        serde_json::Value::Object(_) => vec![&value],
        _ => return Vec::new(),
    };
    items
        .into_iter()
        .filter_map(|item| {
            let pid = item.get("Id").and_then(|v| v.as_u64())?;
            let name = item
                .get("ProcessName")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            Some(AppProcessInfo { pid: pid as u32, name })
        })
        .collect()
}

/// 卸载成功后的兜底清理：该应用残留的用户 PATH 条目 + 未删净的目录。
///
/// Scoop 的 `Remove-Item` 遇到被占用的文件会打印警告但退出码仍为 0；
/// `current` 联接已被解开说明 Scoop 确实执行了卸载，此时残留目录用强制删除收尾。
/// `current` 仍在（Scoop 因进程在运行而整体跳过）时不清理，交由强制卸载流程处理。
///
/// `force` 为真时（用户在弹窗中确认过强制卸载）：删除后仍被占用的文件会按剩余条目
/// 查找占用进程并结束（必要时自动重启资源管理器），再重试删除。
fn after_uninstall_cleanup(
    state: &AppState,
    name: &str,
    force: bool,
) -> Option<Box<dyn FnOnce() + Send>> {
    if !postinstall::safe_app_name(name) {
        return None;
    }
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, state)?;
    let app_dir = root.join("apps").join(name);
    Some(Box::new(move || {
        let _ = crate::domain::bootstrap::remove_paths_under_from_user_path(&app_dir);
        let junction = app_dir.join("current");
        if !app_dir.exists() || std::fs::symlink_metadata(&junction).is_ok() {
            return;
        }
        let first = crate::domain::forcedelete::remove_tree_force(&app_dir);
        if first.removed || !force {
            return;
        }
        // 映像映射的 DLL 打开探测发现不了，只能以「实际删除剩余的文件」为准查占用进程。
        let locked: Vec<std::path::PathBuf> = first
            .remaining
            .iter()
            .filter(|p| p.is_file())
            .cloned()
            .collect();
        let candidates = if locked.is_empty() {
            crate::domain::forcedelete::blocked_files_under(&app_dir, 100)
        } else {
            locked
        };
        let holders = crate::domain::forcedelete::lock_holders(&candidates);
        let had_explorer = holders.iter().any(|h| h.is_explorer());
        for holder in &holders {
            let _ = crate::domain::scoopcmd::kill_tree(holder.pid);
        }
        if had_explorer {
            crate::domain::proc::restart_explorer();
        }
        if !holders.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        let _ = crate::domain::forcedelete::remove_tree_force(&app_dir);
    }))
}

#[tauri::command(async)]
pub fn update_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: Option<String>,
    all: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::update_args(name.as_deref(), all)?;
    // 指名更新单个应用 → App 作用域可并发；全部更新/scoop 自更新+buckets → 全局独占
    let scope = match name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(app_name) if !all => TaskScope::App(app_name.to_string()),
        _ => TaskScope::Global,
    };
    spawn_task_scoped(&app, &state, args, task_id, scope, None)
}

#[tauri::command(async)]
pub fn set_hold(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    hold: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::hold_args(&name, hold)?;
    spawn_task_scoped(&app, &state, args, task_id, TaskScope::App(name), None)
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

#[tauri::command(async)]
pub fn register_app_install(
    state: State<'_, AppState>,
    name: String,
) -> Result<Vec<String>, AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    let dir = root.join("apps").join(&name).join("current");
    postinstall::register(&dir, &name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_process_json_array_and_single_object() {
        let list = parse_process_list(r#"[{"Id":123,"ProcessName":"7zFM"},{"Id":456,"ProcessName":"git"}]"#);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].pid, 123);
        assert_eq!(list[0].name, "7zFM");

        let single = parse_process_list(r#"{"Id":9,"ProcessName":"python"}"#);
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].pid, 9);
    }

    #[test]
    fn parses_process_json_noise_to_empty() {
        assert!(parse_process_list("").is_empty());
        assert!(parse_process_list("   \r\n").is_empty());
        assert!(parse_process_list("not json").is_empty());
        assert!(parse_process_list("[]").is_empty());
        assert!(parse_process_list(r#"{"Message":"no"}"#).is_empty());
    }
}
