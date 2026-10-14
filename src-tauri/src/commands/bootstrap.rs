use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::commands::context;
use crate::commands::roots::resolve_scoop_dir;
use crate::commands::runner::spawn_command;
use crate::domain::bootstrap::{self, BundleInfo, PrereqReport};
use crate::domain::scoopcmd::ScoopCommand;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct FinalizeResult {
    pub manifest_count: usize,
    pub path_updated: bool,
    pub warnings: Vec<String>,
}

fn default_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("scoop")))
        .unwrap_or_else(|| PathBuf::from("scoop"))
}

fn bundle_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("resources").join("scoop-bundle.zip"));
        candidates.push(dir.join("scoop-bundle.zip"));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("scoop-bundle.zip"),
    );
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| AppError::Other("未找到内置 Scoop 快照资源".to_string()))
}

#[tauri::command(async)]
pub fn get_scoop_bundle_info(app: AppHandle) -> Result<BundleInfo, AppError> {
    let zip = bundle_path(&app)?;
    let meta = zip.with_file_name("scoop-bundle.json");
    bootstrap::read_bundle_info(&meta)
}

#[tauri::command(async)]
pub fn check_scoop_prereqs(
    state: State<'_, AppState>,
    dir: Option<String>,
) -> Result<PrereqReport, AppError> {
    let ctx = context();
    let default = default_dir();
    let target = dir
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| default.clone());
    let existing = resolve_scoop_dir(&ctx, &state);
    Ok(bootstrap::assemble_report(&target, &default, existing, bootstrap::probe()))
}

#[tauri::command(async)]
pub fn extract_scoop_bundle(
    app: AppHandle,
    state: State<'_, AppState>,
    dir: String,
    task_id: String,
) -> Result<(), AppError> {
    let ctx = context();
    let target = PathBuf::from(&dir);
    bootstrap::dir_acceptable(&target)?;
    let zip = bundle_path(&app)?;
    let staging = bootstrap::staging_dir(&target);
    let probes = bootstrap::probe();
    let (cmd, _script) =
        bootstrap::extract_command(&zip, &staging, probes.tar_available, &ctx.settings_dir())?;
    spawn_command(&app, &state, cmd, task_id)
}

#[tauri::command(async)]
pub fn finalize_scoop_bootstrap(dir: String, add_to_path: bool) -> Result<FinalizeResult, AppError> {
    let ctx = context();
    let target = PathBuf::from(&dir);
    let staging = bootstrap::staging_dir(&target);
    let count = bootstrap::finalize(&target, &staging)?;
    let mut warnings = Vec::new();
    if let Err(e) = crate::domain::bootstrap::set_user_env("SCOOP", &target.display().to_string()) {
        warnings.push(format!("写入 SCOOP 环境变量失败：{e}"));
    }
    if let Err(e) = crate::domain::bootstrap::write_root_path_config(&ctx.user_config_home, &target) {
        warnings.push(format!("写入 root_path 配置失败：{e}"));
    }
    let mut path_updated = false;
    if add_to_path {
        match bootstrap::ensure_shims(&target) {
            Ok(()) => match bootstrap::add_shims_to_user_path(&target.join("shims")) {
                Ok(()) => path_updated = true,
                Err(e) => warnings.push(format!("加入 PATH 失败: {e}")),
            },
            Err(e) => warnings.push(format!("创建 scoop 命令失败: {e}")),
        }
    }
    if let Err(e) = bootstrap::mark_scoop_dir(&target) {
        warnings.push(format!("写入初始化标记失败: {e}"));
    }
    Ok(FinalizeResult {
        manifest_count: count,
        path_updated,
        warnings,
    })
}

#[tauri::command(async)]
pub fn install_scoop_online(
    app: AppHandle,
    state: State<'_, AppState>,
    dir: String,
    proxy: Option<String>,
    task_id: String,
) -> Result<(), AppError> {
    let ctx = context();
    let target = PathBuf::from(&dir);
    bootstrap::dir_acceptable(&target)?;
    let script = bootstrap::download_installer(&ctx.settings_dir(), proxy.as_deref())?;
    let mut args: Vec<String> = vec![
        "-NoProfile".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-File".to_string(),
        script.display().to_string(),
        "-ScoopDir".to_string(),
        target.display().to_string(),
    ];
    if bootstrap::is_elevated() {
        args.push("-RunAsAdmin".to_string());
    }
    if let Some(p) = proxy.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) {
        args.push("-Proxy".to_string());
        args.push(p);
    }
    let cmd = ScoopCommand {
        program: "powershell.exe".to_string(),
        args,
        raw_args: vec![],
        envs: vec![],
        display: "install scoop online".to_string(),
    };
    spawn_command(&app, &state, cmd, task_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dir_is_next_to_executable() {
        let d = default_dir();
        assert_eq!(d.file_name().unwrap(), "scoop");
    }
}
