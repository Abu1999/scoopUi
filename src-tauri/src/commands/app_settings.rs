use serde::Serialize;

use crate::commands::context;
use crate::error::AppError;
use crate::settings;

#[derive(Debug, Clone, Serialize)]
pub struct AppSettings {
    pub theme: String,
}

#[tauri::command(async)]
pub fn get_app_settings() -> Result<AppSettings, AppError> {
    let ctx = context();
    let theme = settings::load(&ctx.settings_dir())
        .theme
        .unwrap_or_else(|| "system".to_string());
    let theme = if matches!(theme.as_str(), "system" | "light" | "dark") {
        theme
    } else {
        "system".to_string()
    };
    Ok(AppSettings { theme })
}

#[tauri::command(async)]
pub fn set_theme(theme: String) -> Result<(), AppError> {
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err(AppError::Other(format!("未知的主题选项: {theme}")));
    }
    let ctx = context();
    let mut s = settings::load(&ctx.settings_dir());
    s.theme = Some(theme);
    settings::save(&ctx.settings_dir(), &s)
}
