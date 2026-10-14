use std::ffi::OsString;
use std::path::{Path, PathBuf};

use tauri::State;

use crate::commands::{context, Context};
use crate::domain::root::{self, RootInfo};
use crate::error::AppError;
use crate::settings;
use crate::state::AppState;

pub fn resolve_scoop_dir(ctx: &Context, state: &AppState) -> Option<PathBuf> {
    let cached = state.scoop_dir.lock().unwrap().clone();
    if let Some(p) = cached {
        if root::validate(&p).is_ok() {
            return Some(p);
        }
        *state.scoop_dir.lock().unwrap() = None;
    }
    let s = settings::load(&ctx.settings_dir());
    if let Some(dir) = s.scoop_dir.as_deref().map(PathBuf::from) {
        if root::validate(&dir).is_ok() {
            *state.scoop_dir.lock().unwrap() = Some(dir.clone());
            return Some(dir);
        }
    }
    let legacy = settings::legacy_active_root(&ctx.settings_dir())
        .map(PathBuf::from)
        .or_else(|| crate::domain::bootstrap::read_scoop_dir().map(PathBuf::from));
    if let Some(dir) = legacy {
        if root::validate(&dir).is_ok() {
            let _ = adopt_inner(ctx, state, &dir);
            return Some(dir);
        }
    }
    None
}

fn adopt_inner(ctx: &Context, state: &AppState, dir: &Path) -> Result<(), AppError> {
    let mut s = settings::load(&ctx.settings_dir());
    s.scoop_dir = Some(dir.display().to_string());
    settings::save(&ctx.settings_dir(), &s)?;
    *state.scoop_dir.lock().unwrap() = Some(dir.to_path_buf());
    #[cfg(not(test))]
    {
        let _ = crate::domain::bootstrap::mark_scoop_dir(dir);
        let _ = crate::domain::bootstrap::set_user_env("SCOOP", &dir.display().to_string());
    }
    Ok(())
}

#[tauri::command(async)]
pub fn get_scoop_env(state: State<'_, AppState>) -> Result<Option<RootInfo>, AppError> {
    let ctx = context();
    Ok(resolve_scoop_dir(&ctx, &state).map(|p| root::info(&p)))
}

#[tauri::command(async)]
pub fn adopt_scoop_dir(path: String, state: State<'_, AppState>) -> Result<RootInfo, AppError> {
    let ctx = context();
    let p = PathBuf::from(path.trim());
    root::validate(&p)?;
    adopt_inner(&ctx, &state, &p)?;
    #[cfg(not(test))]
    {
        let _ = crate::domain::bootstrap::write_root_path_config(&ctx.user_config_home, &p);
    }
    Ok(root::info(&p))
}

pub fn move_scoop_dir_inner(ctx: &Context, state: &AppState, new_dir: &Path) -> Result<RootInfo, AppError> {
    let current = resolve_scoop_dir(ctx, state).ok_or(AppError::NoRoot)?;
    if root::normalized(new_dir) == root::normalized(&current) {
        return Ok(root::info(&current));
    }
    crate::domain::bootstrap::dir_acceptable(new_dir)?;
    if new_dir.starts_with(&current) || current.starts_with(new_dir) {
        return Err(AppError::Other("目标目录不能与当前环境相互包含".to_string()));
    }
    let junctions = collect_current_junctions(&current);
    if let Some(parent) = new_dir.parent() { std::fs::create_dir_all(parent)?; }
    let renamed = match std::fs::rename(&current, new_dir) {
        Ok(_) => true,
        Err(_) => {
            if let Err(e) = copy_tree(&current, new_dir) {
                let _ = std::fs::remove_dir_all(new_dir);
                return Err(e);
            }
            false
        }
    };
    if let Err(e) = rebuild_current_junctions(&junctions, new_dir) {
        if renamed {
            let _ = std::fs::rename(new_dir, &current);
        } else {
            let _ = std::fs::remove_dir_all(new_dir);
        }
        return Err(e);
    }
    if !renamed {
        if let Err(e) = std::fs::remove_dir_all(&current) {
            let _ = std::fs::remove_dir_all(new_dir);
            return Err(AppError::Other(format!("源目录删除失败，已回滚：{e}")));
        }
    }
    if root::validate(new_dir).is_err() {
        return Err(AppError::Other("迁移后目录校验失败".to_string()));
    }
    #[cfg(not(test))]
    {
        let _ = crate::domain::bootstrap::remove_shims_from_user_path(&current.join("shims"));
        let _ = crate::domain::bootstrap::add_shims_to_user_path(&new_dir.join("shims"));
        let _ = crate::domain::bootstrap::set_user_env("SCOOP", &new_dir.display().to_string());
        let _ = crate::domain::bootstrap::mark_scoop_dir(new_dir);
        let _ = crate::domain::bootstrap::write_root_path_config(&ctx.user_config_home, new_dir);
    }
    let mut s = settings::load(&ctx.settings_dir());
    s.scoop_dir = Some(new_dir.display().to_string());
    settings::save(&ctx.settings_dir(), &s)?;
    *state.scoop_dir.lock().unwrap() = Some(new_dir.to_path_buf());
    Ok(root::info(new_dir))
}

#[tauri::command(async)]
pub fn move_scoop_dir(new_dir: String, state: State<'_, AppState>) -> Result<RootInfo, AppError> {
    let ctx = context();
    move_scoop_dir_inner(&ctx, &state, &PathBuf::from(new_dir.trim()))
}

pub fn remove_scoop_env_inner(ctx: &Context, state: &AppState) -> Result<String, AppError> {
    let dir = resolve_scoop_dir(ctx, state).ok_or(AppError::NoRoot)?;
    if dir.exists() { std::fs::remove_dir_all(&dir)?; }
    #[cfg(not(test))]
    {
        let _ = crate::domain::bootstrap::remove_shims_from_user_path(&dir.join("shims"));
        let _ = crate::domain::bootstrap::remove_user_env("SCOOP");
        let _ = crate::domain::bootstrap::clear_scoop_dir();
    }
    let mut s = settings::load(&ctx.settings_dir());
    s.scoop_dir = None;
    settings::save(&ctx.settings_dir(), &s)?;
    *state.scoop_dir.lock().unwrap() = None;
    Ok(dir.display().to_string())
}

#[tauri::command(async)]
pub fn remove_scoop_env(state: State<'_, AppState>) -> Result<String, AppError> {
    let ctx = context();
    remove_scoop_env_inner(&ctx, &state)
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ft.is_symlink() { continue; }
        if ft.is_dir() { copy_tree(&entry.path(), &to)?; }
        else { std::fs::copy(entry.path(), &to)?; }
    }
    Ok(())
}

fn collect_current_junctions(root: &Path) -> Vec<(OsString, PathBuf)> {
    let mut links: Vec<(OsString, PathBuf)> = Vec::new();
    let apps = root.join("apps");
    let Ok(entries) = std::fs::read_dir(&apps) else { return links };
    for app in entries.flatten() {
        let current = app.path().join("current");
        let Ok(meta) = std::fs::symlink_metadata(&current) else { continue };
        if !meta.file_type().is_symlink() { continue; }
        let Ok(target) = std::fs::read_link(&current) else { continue };
        let Ok(rel) = target.strip_prefix(root) else { continue };
        links.push((app.file_name(), rel.to_path_buf()));
    }
    links
}

fn rebuild_current_junctions(links: &[(OsString, PathBuf)], new_root: &Path) -> Result<(), AppError> {
    use std::os::windows::process::CommandExt;
    for (app, rel) in links {
        let new_link = new_root.join("apps").join(app).join("current");
        let new_target = new_root.join(rel);
        if std::fs::symlink_metadata(&new_link).is_ok() {
            std::fs::remove_dir(&new_link)?;
        }
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J", &new_link.display().to_string(), &new_target.display().to_string()])
            .creation_flags(0x08000000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if status.map(|s| !s.success()).unwrap_or(true) {
            return Err(AppError::Other(format!("重建目录联接失败：{}", new_link.display())));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_root(base: &Path) -> PathBuf {
        let root = base.join("scoop");
        for sub in ["apps", "buckets", "shims"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        root
    }

    fn make_ctx(base: &Path) -> Context {
        Context {
            home: base.join("home"),
            appdata: base.join("appdata"),
            local_appdata: base.join("localappdata"),
            program_data: base.join("programdata"),
            user_config_home: base.join("home").join(".config"),
            env_global: None,
            user_config: serde_json::json!({}),
        }
    }

    fn make_junction(link: &Path, target: &Path) {
        use std::os::windows::process::CommandExt;
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J", &link.display().to_string(), &target.display().to_string()])
            .creation_flags(0x08000000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn move_rebuilds_current_junctions() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let src = make_root(tmp.path());
        let version = src.join("apps").join("git").join("1.0.0");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(version.join("manifest.json"), r#"{"version":"1.0"}"#).unwrap();
        make_junction(&src.join("apps").join("git").join("current"), &version);
        settings::save(&ctx.settings_dir(),
            &settings::Settings { scoop_dir: Some(src.display().to_string()), theme: None }).unwrap();
        *state.scoop_dir.lock().unwrap() = Some(src.clone());
        let dst = tmp.path().join("moved").join("scoop");
        move_scoop_dir_inner(&ctx, &state, &dst).unwrap();
        assert!(!src.exists());
        let manifest = std::fs::read_to_string(
            dst.join("apps").join("git").join("current").join("manifest.json"),
        ).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(parsed["version"], "1.0");
        let link = std::fs::read_link(dst.join("apps").join("git").join("current")).unwrap();
        assert!(link.starts_with(&dst));
    }

    #[test]
    fn resolve_reads_scoop_dir_and_caches() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let root = make_root(tmp.path());
        settings::save(&ctx.settings_dir(),
            &settings::Settings { scoop_dir: Some(root.display().to_string()), theme: None }).unwrap();
        let got = resolve_scoop_dir(&ctx, &state).unwrap();
        assert_eq!(root::normalized(&got), root::normalized(&root));
        let cached = state.scoop_dir.lock().unwrap().clone();
        assert_eq!(root::normalized(cached.as_ref().unwrap()), root::normalized(&root));
    }

    #[test]
    fn resolve_adopts_legacy_active_root() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let root = make_root(tmp.path());
        std::fs::create_dir_all(&ctx.settings_dir()).unwrap();
        let json = serde_json::json!({ "active_root": root.display().to_string() });
        std::fs::write(ctx.settings_dir().join("settings.json"), json.to_string()).unwrap();
        let got = resolve_scoop_dir(&ctx, &state).unwrap();
        assert_eq!(root::normalized(&got), root::normalized(&root));
        assert_eq!(settings::load(&ctx.settings_dir()).scoop_dir.as_deref(),
            Some(root.display().to_string().as_str()));
    }

    #[test]
    fn resolve_returns_none_when_nothing_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        assert!(resolve_scoop_dir(&ctx, &state).is_none());
    }

    #[test]
    fn move_scoop_dir_renames_and_updates_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let src = make_root(tmp.path());
        settings::save(&ctx.settings_dir(),
            &settings::Settings { scoop_dir: Some(src.display().to_string()), theme: None }).unwrap();
        *state.scoop_dir.lock().unwrap() = Some(src.clone());
        let dst = tmp.path().join("moved").join("scoop");
        let info = move_scoop_dir_inner(&ctx, &state, &dst).unwrap();
        assert!(dst.join("apps").is_dir());
        assert!(!src.exists());
        assert_eq!(root::normalized(&PathBuf::from(&info.path)), root::normalized(&dst));
        assert_eq!(settings::load(&ctx.settings_dir()).scoop_dir.as_deref(),
            Some(dst.display().to_string().as_str()));
    }

    #[test]
    fn move_rejects_non_empty_target() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let src = make_root(tmp.path());
        settings::save(&ctx.settings_dir(),
            &settings::Settings { scoop_dir: Some(src.display().to_string()), theme: None }).unwrap();
        let dst = tmp.path().join("occupied");
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(dst.join("x.txt"), "x").unwrap();
        assert!(move_scoop_dir_inner(&ctx, &state, &dst).is_err());
        assert!(src.exists());
    }

    #[test]
    fn remove_scoop_env_deletes_dir_and_clears_state() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let src = make_root(tmp.path());
        settings::save(&ctx.settings_dir(),
            &settings::Settings { scoop_dir: Some(src.display().to_string()), theme: None }).unwrap();
        *state.scoop_dir.lock().unwrap() = Some(src.clone());
        let removed = remove_scoop_env_inner(&ctx, &state).unwrap();
        assert_eq!(removed, src.display().to_string());
        assert!(!src.exists());
        assert!(settings::load(&ctx.settings_dir()).scoop_dir.is_none());
        assert!(state.scoop_dir.lock().unwrap().is_none());
    }
}
