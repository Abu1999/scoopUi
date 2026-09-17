use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::{context, roots::resolve_active_root};
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

#[tauri::command(async)]
pub fn run_scoop(
    app: AppHandle,
    state: State<'_, AppState>,
    args: Vec<String>,
    task_id: String,
) -> Result<(), AppError> {
    let ctx = context();
    let root_path = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    let cmd = scoopcmd::build(&root_path, Some(&global), &args)?;
    let spawned = scoopcmd::spawn(&cmd)?;
    state.tasks.lock().unwrap().insert(task_id.clone(), spawned.pid());

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

pub(crate) fn cancel_task_inner(state: &AppState, task_id: &str) -> bool {
    let pid = state.tasks.lock().unwrap().remove(task_id);
    match pid {
        Some(pid) => scoopcmd::kill_tree(pid),
        None => false,
    }
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
    fn cancel_unknown_task_returns_false() {
        let state = AppState::default();
        assert!(!cancel_task_inner(&state, "unknown"));
    }

    #[test]
    fn cancel_live_task_returns_true_and_removes_entry() {
        let cmd = scoopcmd::ScoopCommand {
            program: "cmd.exe".to_string(),
            args: vec!["/C".to_string(), "ping -n 30 127.0.0.1 >nul".to_string()],
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
}
