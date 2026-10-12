use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::AppError;

pub const UNINSTALL_ROOTS: [&str; 3] = [
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegistryApp {
    pub key_path: String,
    pub name: String,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
    pub exe_names: Vec<String>,
    pub managed: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawEntry {
    pub key_path: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
    pub system_component: Option<u32>,
    pub parent_key_name: Option<String>,
}

pub fn is_under_roots(path: &Path, roots: &[PathBuf]) -> bool {
    let normalized = PathBuf::from(crate::domain::root::normalized(path));
    roots
        .iter()
        .any(|r| normalized.starts_with(PathBuf::from(crate::domain::root::normalized(r))))
}

pub fn parse_entry(raw: &RawEntry, known_roots: &[PathBuf]) -> Option<RegistryApp> {
    let name = raw.name.as_deref()?.trim();
    if name.is_empty() || raw.system_component == Some(1) || raw.parent_key_name.is_some() {
        return None;
    }
    let install_location = raw
        .install_location
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let managed = install_location
        .as_deref()
        .map(|loc| is_under_roots(Path::new(loc), known_roots))
        .unwrap_or(false);
    Some(RegistryApp {
        key_path: raw.key_path.clone(),
        name: name.to_string(),
        version: raw.version.clone(),
        publisher: raw.publisher.clone(),
        install_location,
        uninstall_string: raw.uninstall_string.clone(),
        exe_names: Vec::new(),
        managed,
    })
}

pub fn collect_exe_names(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false) {
            if let Some(stem) = path.file_stem() {
                out.push(stem.to_string_lossy().to_lowercase());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn valid_key_path(key: &str) -> bool {
    UNINSTALL_ROOTS.iter().any(|root| {
        key.get(..root.len())
            .map(|prefix| prefix.eq_ignore_ascii_case(root))
            .unwrap_or(false)
            && key.as_bytes().get(root.len()) == Some(&b'\\')
    })
}

#[cfg(not(windows))]
pub fn scan_registry(_known_roots: &[PathBuf]) -> Vec<RegistryApp> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn read_uninstall_string(_key: &str) -> Result<String, AppError> {
    Err(AppError::Other("仅支持 Windows".to_string()))
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows_sys::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
        HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_DWORD, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn read_string(parent: HKEY, subkey: &str, value: &str) -> Option<String> {
        let sub = wide(subkey);
        let val = wide(value);
        let mut size: u32 = 0;
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
        let status = unsafe {
            RegGetValueW(
                parent,
                sub.as_ptr(),
                val.as_ptr(),
                flags,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS || size == 0 {
            return None;
        }
        let mut buf = vec![0u16; (size as usize + 1) / 2];
        let status = unsafe {
            RegGetValueW(
                parent,
                sub.as_ptr(),
                val.as_ptr(),
                flags,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let text = String::from_utf16_lossy(&buf[..len]).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    fn read_dword(parent: HKEY, subkey: &str, value: &str) -> Option<u32> {
        let sub = wide(subkey);
        let val = wide(value);
        let mut data: u32 = 0;
        let mut size: u32 = 4;
        let status = unsafe {
            RegGetValueW(
                parent,
                sub.as_ptr(),
                val.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut data as *mut u32).cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            None
        } else {
            Some(data)
        }
    }

    pub fn scan(known_roots: &[PathBuf]) -> Vec<RegistryApp> {
        let roots: [(HKEY, &str); 3] = [
            (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_LOCAL_MACHINE, r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        ];
        let mut out: Vec<RegistryApp> = Vec::new();
        for (index, (handle, path)) in roots.iter().enumerate() {
            let sub = wide(path);
            let mut hkey: HKEY = std::ptr::null_mut();
            let status = unsafe { RegOpenKeyExW(*handle, sub.as_ptr(), 0, KEY_READ, &mut hkey) };
            if status != ERROR_SUCCESS {
                continue;
            }
            let display_root = UNINSTALL_ROOTS[index];
            let mut i = 0u32;
            loop {
                let mut name_buf = [0u16; 256];
                let mut name_len = name_buf.len() as u32;
                let status = unsafe {
                    RegEnumKeyExW(
                        hkey,
                        i,
                        name_buf.as_mut_ptr(),
                        &mut name_len,
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                if status == ERROR_NO_MORE_ITEMS || status != ERROR_SUCCESS {
                    break;
                }
                i += 1;
                let sub_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                let raw = RawEntry {
                    key_path: format!("{display_root}\\{sub_name}"),
                    name: read_string(hkey, &sub_name, "DisplayName"),
                    version: read_string(hkey, &sub_name, "DisplayVersion"),
                    publisher: read_string(hkey, &sub_name, "Publisher"),
                    install_location: read_string(hkey, &sub_name, "InstallLocation"),
                    uninstall_string: read_string(hkey, &sub_name, "UninstallString"),
                    system_component: read_dword(hkey, &sub_name, "SystemComponent"),
                    parent_key_name: read_string(hkey, &sub_name, "ParentKeyName"),
                };
                if let Some(mut app) = parse_entry(&raw, known_roots) {
                    if let Some(loc) = app.install_location.as_deref() {
                        app.exe_names = collect_exe_names(Path::new(loc));
                    }
                    out.push(app);
                }
            }
            unsafe { RegCloseKey(hkey) };
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    }

    pub fn uninstall_string(key: &str) -> Result<String, AppError> {
        let (hive, sub) = if key.len() > 5 && key[..5].eq_ignore_ascii_case("HKCU\\") {
            (HKEY_CURRENT_USER, &key[5..])
        } else {
            (HKEY_LOCAL_MACHINE, &key[5..])
        };
        let sub = wide(sub);
        let val = wide("UninstallString");
        let mut size: u32 = 0;
        let status = unsafe {
            RegGetValueW(
                hive,
                sub.as_ptr(),
                val.as_ptr(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS || size == 0 {
            return Err(AppError::Other("未找到卸载命令".to_string()));
        }
        let mut buf = vec![0u16; (size as usize + 1) / 2];
        let status = unsafe {
            RegGetValueW(
                hive,
                sub.as_ptr(),
                val.as_ptr(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(AppError::Other("未找到卸载命令".to_string()));
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let text = String::from_utf16_lossy(&buf[..len]).trim().to_string();
        if text.is_empty() {
            Err(AppError::Other("未找到卸载命令".to_string()))
        } else {
            Ok(text)
        }
    }
}

#[cfg(windows)]
pub fn scan_registry(known_roots: &[PathBuf]) -> Vec<RegistryApp> {
    win::scan(known_roots)
}

#[cfg(windows)]
pub fn read_uninstall_string(key: &str) -> Result<String, AppError> {
    if !valid_key_path(key) {
        return Err(AppError::Other(format!("注册表项非法: {key}")));
    }
    win::uninstall_string(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str) -> RawEntry {
        RawEntry {
            key_path: r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\Test".to_string(),
            name: Some(name.to_string()),
            version: Some("1.0".to_string()),
            publisher: Some("Publisher".to_string()),
            install_location: Some(r"C:\Program Files\Test".to_string()),
            uninstall_string: Some(r#""C:\Program Files\Test\uninst.exe""#.to_string()),
            system_component: None,
            parent_key_name: None,
        }
    }

    #[test]
    fn parse_entry_filters_system_and_children() {
        assert!(parse_entry(&raw("App"), &[]).is_some());

        let mut sc = raw("App");
        sc.system_component = Some(1);
        assert!(parse_entry(&sc, &[]).is_none());

        let mut child = raw("App");
        child.parent_key_name = Some("Parent".to_string());
        assert!(parse_entry(&child, &[]).is_none());

        let mut unnamed = raw("   ");
        unnamed.system_component = None;
        assert!(parse_entry(&unnamed, &[]).is_none());
    }

    #[test]
    fn parse_entry_marks_managed_by_known_roots() {
        let roots = vec![std::path::PathBuf::from(r"C:\scoop"), std::path::PathBuf::from(r"C:\ProgramData\scoop")];
        assert!(parse_entry(&raw("App"), &roots).unwrap().managed == false);
        let mut m = raw("App");
        m.install_location = Some(r"C:\scoop\apps\app\current".to_string());
        assert!(parse_entry(&m, &roots).unwrap().managed);
        let mut g = raw("App");
        g.install_location = Some(r"C:\ProgramData\scoop\apps\app\current".to_string());
        assert!(parse_entry(&g, &roots).unwrap().managed);
        let mut none = raw("App");
        none.install_location = None;
        assert!(!parse_entry(&none, &roots).unwrap().managed);
        let mut near = raw("App");
        near.install_location = Some(r"C:\scoop2\apps\app".to_string());
        assert!(!parse_entry(&near, &roots).unwrap().managed);
    }

    #[test]
    fn collect_exe_names_reads_top_level_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Tool.EXE"), "x").unwrap();
        std::fs::write(tmp.path().join("helper.exe"), "x").unwrap();
        std::fs::write(tmp.path().join("readme.txt"), "x").unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("nested.exe"), "x").unwrap();
        assert_eq!(collect_exe_names(tmp.path()), vec!["helper", "tool"]);
        assert!(collect_exe_names(&tmp.path().join("nope")).is_empty());
    }

    #[test]
    fn read_uninstall_string_rejects_foreign_keys() {
        let err = read_uninstall_string(r"HKLM\Software\Other\Uninstall\X").unwrap_err();
        assert!(err.to_string().contains("注册表项非法"), "实际: {err}");
    }

    #[test]
    fn valid_key_path_is_boundary_safe() {
        assert!(valid_key_path(r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\X"));
        assert!(valid_key_path(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\X"));
        assert!(valid_key_path(
            r"HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\X"
        ));
        assert!(!valid_key_path(&"€".repeat(19)));
        assert!(!valid_key_path(""));
        assert!(!valid_key_path(r"HKLM\Software\Other\Uninstall\X"));
    }

    #[test]
    #[ignore]
    fn scans_real_registry() {
        let apps = scan_registry(&[]);
        assert!(!apps.is_empty());
    }
}
