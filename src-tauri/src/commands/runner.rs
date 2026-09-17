use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::context;
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

#[tauri::command]
pub fn run_scoop(
    app: AppHandle,
    state: State<'_, AppState>,
    args: Vec<String>,
    task_id: String,
) -> Result<(), AppError> {
    let root_path = state
        .active_root
        .lock()
        .unwrap()
        .clone()
        .ok_or(AppError::NoRoot)?;
    let ctx = context();
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

#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<bool, AppError> {
    let pid = state.tasks.lock().unwrap().get(&task_id).copied();
    match pid {
        Some(pid) => Ok(scoopcmd::kill_tree(pid)),
        None => Ok(false),
    }
}
