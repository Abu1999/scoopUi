pub mod app_settings;
pub mod apps;
pub mod bootstrap;
pub mod buckets;
pub mod config;
pub mod icons;
pub mod migrate;
pub mod mirrors;
pub mod ops;
pub mod profiles;
pub mod roots;
pub mod runner;
pub mod search;
pub mod updates;

use std::path::{Path, PathBuf};

use crate::paths;

pub struct Context {
    pub appdata: PathBuf,
    pub program_data: PathBuf,
    /// 旧位置（`%USERPROFILE%\.config`）；仅删除环境时用于清理旧 `root_path` 残留。
    #[cfg_attr(test, allow(dead_code))]
    pub user_config_home: PathBuf,
    /// 配置/配置集/图标缓存所在目录（安装目录下 `data`，回退 `%APPDATA%\scoopui`）。
    pub data_dir: PathBuf,
    pub env_global: Option<PathBuf>,
}

pub fn context() -> Context {
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
    let program_data = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    let user_config_home = paths::user_config_home(&home);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from));
    let data_dir = paths::cached_data_dir(exe_dir.as_deref(), &appdata).to_path_buf();
    let env_global = std::env::var_os("SCOOP_GLOBAL").filter(|v| !v.is_empty()).map(PathBuf::from);
    Context { appdata, program_data, user_config_home, data_dir, env_global }
}

impl Context {
    pub fn settings_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }

    /// 配置集目录（`<data>\profiles`）。
    pub fn profiles_dir(&self) -> PathBuf {
        paths::profiles_dir(&self.data_dir)
    }

    /// 图标缓存目录（`<data>\icons`）。
    pub fn icon_cache_dir(&self) -> PathBuf {
        paths::icon_cache_dir(&self.data_dir)
    }

    /// 全局应用目录：`SCOOP_GLOBAL` 优先，其次环境配置（`<环境目录>\config.json`）
    /// 的 `global_path`，最后回退 `%ProgramData%\scoop`。
    pub fn global_dir(&self, root: Option<&Path>) -> PathBuf {
        let config_global_path = root
            .map(|r| crate::domain::configfile::load(&crate::domain::configfile::resolve(r)))
            .and_then(|config| {
                config
                    .get("global_path")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
            });
        crate::domain::root::global_dir(self.env_global.clone(), config_global_path, &self.program_data)
    }
}

pub fn scan_installed(root: &std::path::Path) -> Vec<crate::domain::installed::InstalledApp> {
    let mut apps = crate::domain::installed::scan(&root.join("apps"));
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}
