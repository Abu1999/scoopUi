use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 安装目录内与程序同装同删的数据目录名。
///
/// 布局：`<安装目录>\data\{settings.json, profiles\, icons\}`。
pub const DATA_DIR_NAME: &str = "data";

/// 旧版（遗留）配置目录：`%APPDATA%\scoopui`。
pub fn settings_dir(appdata: &Path) -> PathBuf {
    appdata.join("scoopui")
}

pub fn user_config_home(home: &Path) -> PathBuf {
    home.join(".config")
}

pub fn default_global_root(program_data: &Path) -> PathBuf {
    program_data.join("scoop")
}

/// 数据目录内的配置集目录。
pub fn profiles_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("profiles")
}

/// 数据目录内的图标缓存目录。
pub fn icon_cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("icons")
}

/// 解析数据目录：优先「可执行文件同级 `\data`」，不可写时回退 `%APPDATA%\scoopui`。
///
/// 安装式部署（NSIS `currentUser` 安装到 `%LOCALAPPDATA%\<产品名>`）下 `\data` 可写，
/// 于是配置与程序同装同删；便携式/受限目录（如 Program Files、只读介质）则回退到
/// `%APPDATA%`，避免直接报错导致应用不可用。
pub fn resolve_data_dir(exe_dir: Option<&Path>, appdata: &Path) -> PathBuf {
    if let Some(dir) = exe_dir {
        let candidate = dir.join(DATA_DIR_NAME);
        if writable(&candidate) {
            return candidate;
        }
    }
    settings_dir(appdata)
}

/// 进程内缓存的数据目录。首次解析后固定，避免每条命令都写探针文件。
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 取（并缓存）数据目录。
pub fn cached_data_dir(exe_dir: Option<&Path>, appdata: &Path) -> &'static Path {
    DATA_DIR.get_or_init(|| resolve_data_dir(exe_dir, appdata))
}

/// 目录是否可写：创建目录后写入并删除一个探针文件。
pub fn writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write-probe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 一次性迁移结果。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MigrationReport {
    pub settings_copied: bool,
    pub profiles_copied: usize,
}

/// 把旧版 `%APPDATA%\scoopui` 下的配置与配置集迁移到新数据目录。
///
/// 仅在「新目录尚无 `settings.json`」时执行，且只补齐缺失文件、绝不覆盖已有文件，
/// 因此可安全重复调用；迁移后保留旧目录（由卸载流程负责清理），避免误删用户数据。
pub fn migrate_legacy_data(legacy: &Path, data_dir: &Path) -> MigrationReport {
    let mut report = MigrationReport::default();
    if legacy == data_dir || data_dir.join("settings.json").is_file() {
        return report;
    }
    let legacy_settings = legacy.join("settings.json");
    let legacy_profiles = profiles_dir(legacy);
    if !legacy_settings.is_file() && !legacy_profiles.is_dir() {
        return report;
    }
    if std::fs::create_dir_all(data_dir).is_err() {
        return report;
    }
    if legacy_settings.is_file() && !data_dir.join("settings.json").exists() {
        if std::fs::copy(&legacy_settings, data_dir.join("settings.json")).is_ok() {
            report.settings_copied = true;
        }
    }
    if legacy_profiles.is_dir() {
        let target = profiles_dir(data_dir);
        if std::fs::create_dir_all(&target).is_ok() {
            if let Ok(entries) = std::fs::read_dir(&legacy_profiles) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let dest = target.join(entry.file_name());
                    if dest.exists() {
                        continue;
                    }
                    if std::fs::copy(&path, &dest).is_ok() {
                        report.profiles_copied += 1;
                    }
                }
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_expected_subpaths() {
        let base = PathBuf::from("C:\\Users\\u\\AppData\\Roaming");
        assert_eq!(settings_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\scoopui"));
        assert_eq!(user_config_home(Path::new("C:\\Users\\u")), PathBuf::from("C:\\Users\\u\\.config"));
        assert_eq!(default_global_root(Path::new("C:\\ProgramData")), PathBuf::from("C:\\ProgramData\\scoop"));
        assert_eq!(profiles_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\profiles"));
        assert_eq!(icon_cache_dir(&base), PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\icons"));
    }

    #[test]
    fn prefers_portable_data_dir_when_writable() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("install");
        std::fs::create_dir_all(&exe).unwrap();
        let data = resolve_data_dir(Some(&exe), &tmp.path().join("appdata"));
        assert_eq!(data, exe.join("data"));
        assert!(data.is_dir());
        // 探针文件不应残留
        assert!(!data.join(".write-probe").exists());
    }

    #[test]
    fn falls_back_to_appdata_without_exe_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let appdata = tmp.path().join("appdata");
        assert_eq!(resolve_data_dir(None, &appdata), appdata.join("scoopui"));
    }

    #[test]
    fn falls_back_when_exe_dir_not_writable() {
        let tmp = tempfile::tempdir().unwrap();
        // 用一个「文件」占住 exe 位置，使 <exe>\data 无法创建
        let exe = tmp.path().join("install");
        std::fs::write(&exe, "x").unwrap();
        let appdata = tmp.path().join("appdata");
        assert_eq!(resolve_data_dir(Some(&exe), &appdata), appdata.join("scoopui"));
    }

    #[test]
    fn migrate_copies_settings_and_profiles_once() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("appdata").join("scoopui");
        std::fs::create_dir_all(profiles_dir(&legacy)).unwrap();
        std::fs::write(legacy.join("settings.json"), r#"{"scoop_dir":"D:\\scoop"}"#).unwrap();
        std::fs::write(profiles_dir(&legacy).join("工作.json"), "{}").unwrap();
        let data = tmp.path().join("install").join("data");

        let report = migrate_legacy_data(&legacy, &data);
        assert!(report.settings_copied);
        assert_eq!(report.profiles_copied, 1);
        assert!(data.join("settings.json").is_file());
        assert!(profiles_dir(&data).join("工作.json").is_file());

        // 二次调用：新目录已有 settings.json，直接跳过
        let again = migrate_legacy_data(&legacy, &data);
        assert_eq!(again, MigrationReport::default());
    }

    #[test]
    fn migrate_never_overwrites_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("legacy");
        std::fs::create_dir_all(profiles_dir(&legacy)).unwrap();
        std::fs::write(legacy.join("settings.json"), r#"{"scoop_dir":"OLD"}"#).unwrap();
        let data = tmp.path().join("data");
        std::fs::create_dir_all(profiles_dir(&data)).unwrap();
        std::fs::write(profiles_dir(&data).join("keep.json"), "KEEP").unwrap();

        // 新目录无 settings.json → 会迁移 settings，但同名 profile 不覆盖
        std::fs::write(profiles_dir(&legacy).join("keep.json"), "NEW").unwrap();
        let report = migrate_legacy_data(&legacy, &data);
        assert_eq!(report.profiles_copied, 0);
        assert_eq!(
            std::fs::read_to_string(profiles_dir(&data).join("keep.json")).unwrap(),
            "KEEP"
        );
    }

    #[test]
    fn migrate_noop_when_nothing_legacy() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("legacy");
        let data = tmp.path().join("data");
        assert_eq!(migrate_legacy_data(&legacy, &data), MigrationReport::default());
        assert!(!data.exists());
    }
}
