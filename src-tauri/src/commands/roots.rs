use std::path::{Path, PathBuf};

use tauri::State;

use crate::commands::{context, Context};
use crate::domain::root::{self, RootInfo};
use crate::error::AppError;
use crate::settings;
use crate::state::AppState;

pub fn resolve_active_root(ctx: &Context, state: &AppState) -> Option<PathBuf> {
    let cached = state.active_root.lock().unwrap().clone();
    if let Some(p) = cached {
        if root::validate(&p).is_ok() {
            return Some(p);
        }
        *state.active_root.lock().unwrap() = None;
    }
    let stored = settings::load(&ctx.settings_dir()).active_root.map(PathBuf::from);
    if let Some(p) = stored {
        if root::validate(&p).is_ok() {
            *state.active_root.lock().unwrap() = Some(p.clone());
            return Some(p);
        }
    }
    let probed = root::candidates(
        ctx.env_scoop.clone(),
        ctx.config_root_path(),
        &known_roots(ctx),
        &ctx.home,
    )
    .into_iter()
    .map(|c| c.path)
    .find(|p| root::validate(p).is_ok())?;
    *state.active_root.lock().unwrap() = Some(probed.clone());
    let mut s = settings::load(&ctx.settings_dir());
    s.active_root = Some(probed.display().to_string());
    let _ = settings::save(&ctx.settings_dir(), &s);
    Some(probed)
}

fn known_roots(ctx: &Context) -> Vec<PathBuf> {
    settings::load(&ctx.settings_dir())
        .known_roots
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn build_infos(ctx: &Context, state: &AppState) -> Vec<RootInfo> {
    let known = known_roots(ctx);
    let active = resolve_active_root(ctx, state);
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    root::candidates(ctx.env_scoop.clone(), ctx.config_root_path(), &known, &ctx.home)
        .iter()
        .map(|c| {
            let is_active = active
                .as_ref()
                .map(|a| root::normalized(a) == root::normalized(&c.path))
                .unwrap_or(false);
            root::info(&c.path, c.source, is_active, Some(&global))
        })
        .collect()
}

#[tauri::command(async)]
pub fn detect_roots(state: State<'_, AppState>) -> Result<Vec<RootInfo>, AppError> {
    let ctx = context();
    Ok(build_infos(&ctx, &state))
}

#[tauri::command(async)]
pub fn get_active_root(state: State<'_, AppState>) -> Result<Option<RootInfo>, AppError> {
    let ctx = context();
    match resolve_active_root(&ctx, &state) {
        Some(path) => {
            let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
            Ok(Some(root::info(&path, "settings", true, Some(&global))))
        }
        None => Ok(None),
    }
}

#[tauri::command(async)]
pub fn set_active_root(path: String, state: State<'_, AppState>) -> Result<RootInfo, AppError> {
    let ctx = context();
    let p = PathBuf::from(&path);
    root::validate(&p)?;
    let mut s = settings::load(&ctx.settings_dir());
    s.active_root = Some(path.clone());
    if !s.known_roots.iter().any(|k| root::normalized(Path::new(k)) == root::normalized(&p)) {
        s.known_roots.push(path.clone());
    }
    settings::save(&ctx.settings_dir(), &s)?;
    *state.active_root.lock().unwrap() = Some(p.clone());
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    Ok(root::info(&p, "manual", true, Some(&global)))
}

#[tauri::command(async)]
pub fn add_root(path: String, state: State<'_, AppState>) -> Result<RootInfo, AppError> {
    let ctx = context();
    let p = PathBuf::from(&path);
    root::validate(&p)?;
    let mut s = settings::load(&ctx.settings_dir());
    if !s.known_roots.iter().any(|k| root::normalized(Path::new(k)) == root::normalized(&p)) {
        s.known_roots.push(path.clone());
    }
    settings::save(&ctx.settings_dir(), &s)?;
    let is_active = resolve_active_root(&ctx, &state)
        .map(|a| root::normalized(&a) == root::normalized(&p))
        .unwrap_or(false);
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    Ok(root::info(&p, "manual", is_active, Some(&global)))
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
            program_data: base.join("programdata"),
            user_config_home: base.join("home").join(".config"),
            env_scoop: None,
            env_global: None,
            user_config: serde_json::json!({}),
        }
    }

    fn store_active(ctx: &Context, path: &str) {
        settings::save(
            &ctx.settings_dir(),
            &settings::Settings {
                known_roots: vec![],
                active_root: Some(path.to_string()),
            },
        )
        .unwrap();
    }

    #[test]
    fn resolve_hydrates_from_settings_and_caches() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let root = make_root(tmp.path());
        store_active(&ctx, &root.display().to_string());
        let got = resolve_active_root(&ctx, &state).unwrap();
        assert_eq!(root::normalized(&got), root::normalized(&root));
        let cached = state.active_root.lock().unwrap().clone();
        assert_eq!(root::normalized(cached.as_ref().unwrap()), root::normalized(&root));
        std::fs::remove_file(ctx.settings_dir().join("settings.json")).unwrap();
        let again = resolve_active_root(&ctx, &state).unwrap();
        assert_eq!(root::normalized(&again), root::normalized(&root));
    }

    #[test]
    fn resolve_prefers_memory_over_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let memory = make_root(&tmp.path().join("mem"));
        let stored = make_root(&tmp.path().join("stored"));
        *state.active_root.lock().unwrap() = Some(memory.clone());
        store_active(&ctx, &stored.display().to_string());
        let got = resolve_active_root(&ctx, &state).unwrap();
        assert_eq!(root::normalized(&got), root::normalized(&memory));
    }

    #[test]
    fn resolve_returns_none_for_invalid_settings_path() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        store_active(&ctx, &tmp.path().join("missing").display().to_string());
        assert!(resolve_active_root(&ctx, &state).is_none());
        assert!(state.active_root.lock().unwrap().is_none());
    }

    #[test]
    fn resolve_clears_stale_memory() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let root = make_root(tmp.path());
        *state.active_root.lock().unwrap() = Some(root.clone());
        std::fs::remove_dir_all(&root).unwrap();
        assert!(resolve_active_root(&ctx, &state).is_none());
        assert!(state.active_root.lock().unwrap().is_none());
    }

    #[test]
    fn resolve_falls_back_to_settings_when_memory_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let stale = tmp.path().join("stale");
        let root = make_root(tmp.path());
        *state.active_root.lock().unwrap() = Some(stale);
        store_active(&ctx, &root.display().to_string());
        let got = resolve_active_root(&ctx, &state).unwrap();
        assert_eq!(root::normalized(&got), root::normalized(&root));
        let cached = state.active_root.lock().unwrap().clone();
        assert_eq!(root::normalized(cached.as_ref().unwrap()), root::normalized(&root));
    }

    #[test]
    fn resolve_auto_activates_probed_default_root() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        let root = make_root(&ctx.home);
        let got = resolve_active_root(&ctx, &state).unwrap();
        assert_eq!(root::normalized(&got), root::normalized(&root));
        let cached = state.active_root.lock().unwrap().clone();
        assert_eq!(root::normalized(cached.as_ref().unwrap()), root::normalized(&root));
        let saved = settings::load(&ctx.settings_dir());
        assert_eq!(
            saved.active_root.as_deref().map(|s| root::normalized(Path::new(s))),
            Some(root::normalized(&root))
        );
        std::fs::remove_file(ctx.settings_dir().join("settings.json")).unwrap();
        let again = resolve_active_root(&ctx, &state).unwrap();
        assert_eq!(root::normalized(&again), root::normalized(&root));
    }

    #[test]
    fn resolve_returns_none_when_no_root_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let state = AppState::default();
        assert!(resolve_active_root(&ctx, &state).is_none());
        assert!(state.active_root.lock().unwrap().is_none());
        assert!(!ctx.settings_dir().join("settings.json").exists());
    }

    #[test]
    fn resolve_treats_trailing_separator_equivalently() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let root = make_root(tmp.path());
        let with_sep = AppState::default();
        store_active(&ctx, &format!("{}\\", root.display()));
        let a = resolve_active_root(&ctx, &with_sep).unwrap();
        let no_sep = AppState::default();
        store_active(&ctx, &root.display().to_string());
        let b = resolve_active_root(&ctx, &no_sep).unwrap();
        assert_eq!(root::normalized(&a), root::normalized(&b));
        assert_eq!(root::normalized(&a), root::normalized(&root));
    }
}
