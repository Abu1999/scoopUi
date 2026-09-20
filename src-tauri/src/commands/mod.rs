pub mod apps;
pub mod buckets;
pub mod config;
pub mod icons;
pub mod ops;
pub mod roots;
pub mod runner;
pub mod search;
pub mod updates;

use std::path::PathBuf;

use crate::paths;

pub struct Context {
    pub home: PathBuf,
    pub appdata: PathBuf,
    pub local_appdata: PathBuf,
    pub program_data: PathBuf,
    pub user_config_home: PathBuf,
    pub env_scoop: Option<PathBuf>,
    pub env_global: Option<PathBuf>,
    pub user_config: serde_json::Value,
}

pub fn context() -> Context {
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
    let local_appdata = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| appdata.clone());
    let program_data = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    let user_config_home = paths::user_config_home(&home);
    let env_scoop = std::env::var_os("SCOOP").filter(|v| !v.is_empty()).map(PathBuf::from);
    let env_global = std::env::var_os("SCOOP_GLOBAL").filter(|v| !v.is_empty()).map(PathBuf::from);
    let user_config = crate::domain::configfile::load(&user_config_home.join("scoop").join("config.json"));
    Context { home, appdata, local_appdata, program_data, user_config_home, env_scoop, env_global, user_config }
}

impl Context {
    pub fn config_root_path(&self) -> Option<PathBuf> {
        self.user_config
            .get("root_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    }

    pub fn config_global_path(&self) -> Option<PathBuf> {
        self.user_config
            .get("global_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    }

    pub fn settings_dir(&self) -> PathBuf {
        paths::settings_dir(&self.appdata)
    }
}
