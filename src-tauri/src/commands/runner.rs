use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::{context, roots::resolve_scoop_dir};
use crate::domain::root;
use crate::domain::scoopcmd;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogPayload {
    task_id: String,
    stream: String,
    line: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EndPayload {
    task_id: String,
    code: Option<i32>,
    success: bool,
}

pub(crate) fn ensure_idle(tasks: &HashMap<String, u32>) -> Result<(), AppError> {
    if tasks.is_empty() {
        Ok(())
    } else {
        Err(AppError::Other("已有任务正在运行，请等待完成或取消后再试".to_string()))
    }
}

pub(crate) fn spawn_command(
    app: &AppHandle,
    state: &AppState,
    cmd: scoopcmd::ScoopCommand,
    task_id: String,
) -> Result<(), AppError> {
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

pub(crate) fn spawn_task(
    app: &AppHandle,
    state: &AppState,
    args: Vec<String>,
    task_id: String,
) -> Result<(), AppError> {
    let ctx = context();
    let root_path = resolve_scoop_dir(&ctx, state).ok_or(AppError::NoRoot)?;
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    let cmd = scoopcmd::build(&root_path, Some(&global), &args)?;
    spawn_command(app, state, cmd, task_id)
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

pub(crate) fn cancel_task_inner(state: &AppState, task_id: &str) -> bool {
    let pid = match state.tasks.lock().unwrap().get(task_id).copied() {
        Some(pid) => pid,
        None => return false,
    };
    let ok = scoopcmd::kill_tree(pid);
    if ok {
        state.tasks.lock().unwrap().remove(task_id);
    }
    ok
}

#[tauri::command(async)]
pub fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<bool, AppError> {
    Ok(cancel_task_inner(&state, &task_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_payload_field_names() {
        let v = serde_json::to_value(LogPayload {
            task_id: "t1".to_string(),
            stream: "stdout".to_string(),
            line: "hello".to_string(),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"taskId": "t1", "stream": "stdout", "line": "hello"})
        );
    }

    #[test]
    fn end_payload_field_names_with_code() {
        let v = serde_json::to_value(EndPayload {
            task_id: "t1".to_string(),
            code: Some(1),
            success: true,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"taskId": "t1", "code": 1, "success": true}));
    }

    #[test]
    fn end_payload_none_code_is_null() {
        let v = serde_json::to_value(EndPayload {
            task_id: "t1".to_string(),
            code: None,
            success: false,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({"taskId": "t1", "code": null, "success": false}));
    }

    #[test]
    fn ensure_idle_rejects_when_task_running() {
        let mut tasks = std::collections::HashMap::new();
        assert!(ensure_idle(&tasks).is_ok());
        tasks.insert("t".to_string(), 1234);
        let err = ensure_idle(&tasks).unwrap_err();
        assert!(err.to_string().contains("已有任务正在运行"));
    }

    #[test]
    fn cancel_unknown_task_returns_false() {
        let state = AppState::default();
        assert!(!cancel_task_inner(&state, "unknown"));
    }

    #[test]
    fn cancel_live_task_returns_true_and_removes_entry() {
        let cmd = scoopcmd::ScoopCommand {
            program: "cmd.exe".to_string(),
            args: vec!["/C".to_string(), "ping -n 30 127.0.0.1 >nul".to_string()],
            raw_args: vec![],
            envs: vec![],
            display: "cancel".to_string(),
        };
        let mut spawned = scoopcmd::spawn(&cmd).unwrap();
        let state = AppState::default();
        state.tasks.lock().unwrap().insert("task-1".to_string(), spawned.pid());
        assert!(cancel_task_inner(&state, "task-1"));
        assert!(!state.tasks.lock().unwrap().contains_key("task-1"));
        let start = std::time::Instant::now();
        let _ = spawned.child.wait();
        assert!(start.elapsed().as_secs() < 10);
    }

    #[test]
    fn cancel_dead_task_returns_false_and_keeps_entry() {
        let cmd = scoopcmd::ScoopCommand {
            program: "cmd.exe".to_string(),
            args: vec!["/C".to_string(), "exit /b 0".to_string()],
            raw_args: vec![],
            envs: vec![],
            display: "dead".to_string(),
        };
        let mut spawned = scoopcmd::spawn(&cmd).unwrap();
        let pid = spawned.pid();
        let _ = spawned.child.wait();
        let state = AppState::default();
        state.tasks.lock().unwrap().insert("task-dead".to_string(), pid);
        assert!(!cancel_task_inner(&state, "task-dead"));
        assert!(state.tasks.lock().unwrap().contains_key("task-dead"));
    }
}
