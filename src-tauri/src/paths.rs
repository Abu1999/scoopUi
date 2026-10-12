use std::path::{Path, PathBuf};

pub fn settings_dir(appdata: &Path) -> PathBuf {
    appdata.join("scoopui")
}

pub fn icon_cache_dir(localappdata: &Path) -> PathBuf {
    localappdata.join("scoopui").join("icons")
}

pub fn user_config_home(home: &Path) -> PathBuf {
    home.join(".config")
}

pub fn default_root(home: &Path) -> PathBuf {
    home.join("scoop")
}

pub fn default_global_root(program_data: &Path) -> PathBuf {
    program_data.join("scoop")
}

pub fn profiles_dir(appdata: &Path) -> PathBuf {
    settings_dir(appdata).join("profiles")
}

pub fn snapshots_dir(appdata: &Path) -> PathBuf {
    settings_dir(appdata).join("snapshots")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_expected_subpaths() {
        let base = PathBuf::from("C:\\Users\\u\\AppData\\Roaming");
        assert_eq!(settings_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\scoopui"));
        assert_eq!(icon_cache_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\scoopui\\icons"));
        assert_eq!(user_config_home(Path::new("C:\\Users\\u")), PathBuf::from("C:\\Users\\u\\.config"));
        assert_eq!(default_root(Path::new("C:\\Users\\u")), PathBuf::from("C:\\Users\\u\\scoop"));
        assert_eq!(default_global_root(Path::new("C:\\ProgramData")), PathBuf::from("C:\\ProgramData\\scoop"));
        assert_eq!(profiles_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\scoopui\\profiles"));
        assert_eq!(snapshots_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\scoopui\\snapshots"));
    }
}
