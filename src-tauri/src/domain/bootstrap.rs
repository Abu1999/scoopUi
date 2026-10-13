use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::domain::scoopcmd::ScoopCommand;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BundleInfo {
    pub built: String,
    pub scoop_commit: String,
    pub main_commit: String,
}

#[derive(serde::Deserialize, Default)]
struct RawRepo {
    #[serde(default)]
    commit: String,
}

#[derive(serde::Deserialize, Default)]
struct RawBundle {
    #[serde(default)]
    built: String,
    #[serde(default)]
    scoop: RawRepo,
    #[serde(default)]
    main: RawRepo,
}

pub fn read_bundle_info(path: &Path) -> Result<BundleInfo, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|_| AppError::Other(format!("未找到内置快照元数据: {}", path.display())))?;
    let text = text.trim_start_matches('\u{feff}');
    let raw: RawBundle = serde_json::from_str(text)
        .map_err(|_| AppError::Other("内置快照元数据损坏".to_string()))?;
    if raw.scoop.commit.is_empty() || raw.main.commit.is_empty() {
        return Err(AppError::Other("内置快照元数据损坏".to_string()));
    }
    Ok(BundleInfo {
        built: raw.built,
        scoop_commit: raw.scoop.commit,
        main_commit: raw.main.commit,
    })
}

pub fn dir_acceptable(dir: &Path) -> Result<(), AppError> {
    if dir.is_file() {
        return Err(AppError::Other(format!("目标是一个文件: {}", dir.display())));
    }
    if dir.is_dir() {
        if std::fs::read_dir(dir)?.flatten().next().is_some() {
            return Err(AppError::Other(format!("目标目录非空: {}", dir.display())));
        }
    }
    Ok(())
}

pub fn staging_dir(dir: &Path) -> PathBuf {
    match dir.file_name() {
        Some(name) => {
            let mut sibling = name.to_os_string();
            sibling.push(".scoopui-extract");
            dir.with_file_name(sibling)
        }
        None => dir.join(".scoopui-extract"),
    }
}

pub fn tar_program() -> String {
    if let Some(root) = std::env::var_os("SystemRoot") {
        let p = PathBuf::from(root).join("System32").join("tar.exe");
        if p.is_file() {
            return p.display().to_string();
        }
    }
    "tar.exe".to_string()
}

pub fn extract_command(
    zip: &Path,
    staging: &Path,
    tar_available: bool,
    script_dir: &Path,
) -> Result<(ScoopCommand, Option<PathBuf>), AppError> {
    if staging.exists() && !staging_owned(staging) {
        return Err(AppError::Other(format!(
            "暂存目录被其他内容占用: {}",
            staging.display()
        )));
    }
    let _ = std::fs::remove_dir_all(staging);
    std::fs::create_dir_all(staging)?;
    std::fs::write(staging.join(".scoopui-staging"), "scoopui")?;
    if tar_available {
        let cmd = ScoopCommand {
            program: tar_program(),
            args: vec![
                "-xf".to_string(),
                zip.display().to_string(),
                "-C".to_string(),
                staging.display().to_string(),
            ],
            raw_args: vec![],
            envs: vec![],
            display: "extract scoop bundle".to_string(),
        };
        return Ok((cmd, None));
    }
    std::fs::create_dir_all(script_dir)?;
    let script = script_dir.join("expand-scoop-bundle.ps1");
    let text = format!(
        "$ErrorActionPreference = 'Stop'\r\nExpand-Archive -LiteralPath {} -DestinationPath {} -Force\r\nexit $LASTEXITCODE\r\n",
        crate::domain::scoopcmd::quote_ps(&zip.display().to_string()),
        crate::domain::scoopcmd::quote_ps(&staging.display().to_string())
    );
    let mut bytes = vec![0xEFu8, 0xBB, 0xBF];
    bytes.extend_from_slice(text.as_bytes());
    std::fs::write(&script, bytes)?;
    let cmd = ScoopCommand {
        program: "powershell.exe".to_string(),
        args: vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            script.display().to_string(),
        ],
        raw_args: vec![],
        envs: vec![],
        display: "extract scoop bundle (Expand-Archive)".to_string(),
    };
    Ok((cmd, Some(script)))
}

fn staging_owned(staging: &Path) -> bool {
    staging.join(".scoopui-staging").is_file()
        || std::fs::read_dir(staging)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false)
}

pub fn finalize(dir: &Path, staging: &Path) -> Result<usize, AppError> {
    if !staging.is_dir() {
        return Err(AppError::Other(format!(
            "未找到解压暂存目录: {}",
            staging.display()
        )));
    }
    let src_apps = staging.join("apps");
    let src_buckets = staging.join("buckets");
    if !src_apps.is_dir() || !src_buckets.is_dir() {
        return Err(AppError::Other("解压内容不完整（缺少 apps/buckets）".to_string()));
    }
    let dst_apps = dir.join("apps");
    let dst_buckets = dir.join("buckets");
    if dst_apps.exists() || dst_buckets.exists() {
        return Err(AppError::Other(format!(
            "目标目录已包含 apps/buckets: {}",
            dir.display()
        )));
    }
    std::fs::create_dir_all(dir)?;
    let result = (|| -> Result<usize, AppError> {
        std::fs::rename(&src_apps, &dst_apps)?;
        std::fs::rename(&src_buckets, &dst_buckets)?;
        let scoop_ps1 = dst_apps.join("scoop").join("current").join("bin").join("scoop.ps1");
        let manifest_count =
            crate::domain::buckets::count_json_recursive(&dst_buckets.join("main").join("bucket"));
        if !scoop_ps1.is_file() || manifest_count == 0 {
            return Err(AppError::Other("内置快照内容不完整".to_string()));
        }
        for sub in ["cache", "persist", "shims"] {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        Ok(manifest_count)
    })();
    match result {
        Ok(count) => {
            let _ = std::fs::remove_dir_all(staging);
            Ok(count)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dst_apps);
            let _ = std::fs::remove_dir_all(&dst_buckets);
            for sub in ["cache", "persist", "shims"] {
                let _ = std::fs::remove_dir_all(dir.join(sub));
            }
            let _ = std::fs::remove_dir_all(staging);
            Err(e)
        }
    }
}

pub fn write_root_path_config(user_config_home: &Path, dir: &Path) -> Result<(), AppError> {
    let config = user_config_home.join("scoop").join("config.json");
    crate::domain::configfile::set_key(
        &config,
        "root_path",
        Some(serde_json::Value::String(dir.display().to_string())),
    )
}

pub fn ensure_shims(dir: &Path) -> Result<(), AppError> {
    let shims = dir.join("shims");
    std::fs::create_dir_all(&shims)?;
    let ps1 = concat!(
        "$path = Join-Path $PSScriptRoot \"..\\apps\\scoop\\current\\bin\\scoop.ps1\"\r\n",
        "if ($MyInvocation.ExpectingInput) { $input | & $path @args } else { & $path @args }\r\n",
        "exit $LASTEXITCODE\r\n"
    );
    std::fs::write(shims.join("scoop.ps1"), ps1)?;
    let cmd = concat!(
        "@echo off\r\n",
        "powershell -noprofile -ex unrestricted -file \"%~dp0..\\apps\\scoop\\current\\bin\\scoop.ps1\" %*\r\n"
    );
    std::fs::write(shims.join("scoop.cmd"), cmd)?;
    Ok(())
}

pub fn path_contains(path_var: &str, shims: &Path) -> bool {
    let target = crate::domain::root::normalized(shims);
    path_var
        .split(';')
        .any(|p| !p.trim().is_empty() && crate::domain::root::normalized(Path::new(p.trim())) == target)
}

pub fn merge_path_var(current: &str, shims: &Path) -> String {
    if path_contains(current, shims) {
        return current.to_string();
    }
    let entry = shims.display().to_string();
    if current.trim().is_empty() {
        entry
    } else {
        format!("{entry};{current}")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ProbeResults {
    pub powershell_major: Option<u32>,
    pub tar_available: bool,
    pub git_available: bool,
    pub elevated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrereqReport {
    pub default_dir: String,
    pub dir: String,
    pub dir_ok: bool,
    pub dir_reason: Option<String>,
    pub powershell_ok: bool,
    pub tar_available: bool,
    pub git_available: bool,
    pub elevated: bool,
    pub existing_root: Option<String>,
}

fn command_available(program: &str) -> bool {
    std::process::Command::new(program)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn powershell_major() -> Option<u32> {
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", "$PSVersionTable.PSVersion.Major"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

pub fn probe() -> ProbeResults {
    ProbeResults {
        powershell_major: powershell_major(),
        tar_available: command_available(&tar_program()),
        git_available: command_available("git"),
        elevated: is_elevated(),
    }
}

pub fn assemble_report(
    dir: &Path,
    default_dir: &Path,
    existing_root: Option<PathBuf>,
    probes: ProbeResults,
) -> PrereqReport {
    let (dir_ok, dir_reason) = match dir_acceptable(dir) {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e.to_string())),
    };
    PrereqReport {
        default_dir: default_dir.display().to_string(),
        dir: dir.display().to_string(),
        dir_ok,
        dir_reason,
        powershell_ok: probes.powershell_major.map(|m| m >= 5).unwrap_or(false),
        tar_available: probes.tar_available,
        git_available: probes.git_available,
        elevated: probes.elevated,
        existing_root: existing_root.map(|p| p.display().to_string()),
    }
}

pub fn validate_installer_text(text: &str) -> Result<(), AppError> {
    let ok = text.len() > 1000 && text.contains("$ScoopDir") && text.contains("Install-Scoop");
    if ok {
        Ok(())
    } else {
        Err(AppError::Other("下载的安装脚本内容异常".to_string()))
    }
}

pub fn download_installer(settings_dir: &Path, proxy: Option<&str>) -> Result<PathBuf, AppError> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("ScoopUI/0.1");
    if let Some(p) = proxy.map(str::trim).filter(|p| !p.is_empty()) {
        builder = builder
            .proxy(reqwest::Proxy::all(p).map_err(|e| AppError::Other(format!("代理无效: {e}")))?);
    }
    let client = builder
        .build()
        .map_err(|e| AppError::Other(format!("HTTP 客户端初始化失败: {e}")))?;
    let resp = client
        .get("https://get.scoop.sh")
        .send()
        .map_err(|e| AppError::Other(format!("下载 Scoop 安装脚本失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Other(format!(
            "下载 Scoop 安装脚本失败: HTTP {}",
            resp.status()
        )));
    }
    let text = resp
        .text()
        .map_err(|e| AppError::Other(format!("读取安装脚本失败: {e}")))?;
    validate_installer_text(&text)?;
    std::fs::create_dir_all(settings_dir)?;
    let path = settings_dir.join("install-scoop.ps1");
    std::fs::write(&path, text)?;
    Ok(path)
}

#[cfg(windows)]
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

#[cfg(windows)]
pub fn add_shims_to_user_path(shims: &Path) -> Result<(), AppError> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ, REG_VALUE_TYPE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe {
        let sub = wide("Environment");
        let mut hkey: HKEY = std::ptr::null_mut();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut hkey,
        );
        if status != 0 {
            return Err(AppError::Other("无法打开用户环境变量注册表项".to_string()));
        }
        let name = wide("Path");
        let mut value_type: REG_VALUE_TYPE = REG_SZ;
        let mut size: u32 = 0;
        let query = RegQueryValueExW(
            hkey,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut value_type,
            std::ptr::null_mut(),
            &mut size,
        );
        let mut current = String::new();
        if query == 0 && size > 0 {
            let mut buf = vec![0u8; size as usize];
            let query2 = RegQueryValueExW(
                hkey,
                name.as_ptr(),
                std::ptr::null_mut(),
                &mut value_type,
                buf.as_mut_ptr(),
                &mut size,
            );
            if query2 != 0 {
                RegCloseKey(hkey);
                return Err(AppError::Other("读取用户 PATH 失败".to_string()));
            }
            let len = size as usize / 2;
            let mut utf16 = Vec::with_capacity(len);
            for chunk in buf.chunks_exact(2).take(len) {
                utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
            }
            while utf16.last() == Some(&0) {
                utf16.pop();
            }
            current = String::from_utf16_lossy(&utf16).to_string();
        } else if query != 0 && query != 2 {
            RegCloseKey(hkey);
            return Err(AppError::Other("读取用户 PATH 失败".to_string()));
        }
        let merged = crate::domain::bootstrap::merge_path_var(&current, shims);
        if merged != current {
            let write_type: REG_VALUE_TYPE = if value_type == REG_EXPAND_SZ || merged.contains('%') {
                REG_EXPAND_SZ
            } else {
                REG_SZ
            };
            let mut data: Vec<u16> = merged.encode_utf16().collect();
            data.push(0);
            let bytes = std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2);
            let status = RegSetValueExW(
                hkey,
                name.as_ptr(),
                0,
                write_type,
                bytes.as_ptr(),
                bytes.len() as u32,
            );
            if status != 0 {
                RegCloseKey(hkey);
                return Err(AppError::Other("写入用户 PATH 失败".to_string()));
            }
        }
        RegCloseKey(hkey);
    }
    let environment = wide("Environment");
    unsafe {
        let mut result: usize = 0;
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            environment.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            3000,
            &mut result,
        );
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn add_shims_to_user_path(_shims: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_acceptable_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("new");
        assert!(dir_acceptable(&missing).is_ok());
        std::fs::create_dir_all(&missing).unwrap();
        assert!(dir_acceptable(&missing).is_ok());
        std::fs::write(missing.join("x.txt"), "x").unwrap();
        assert!(dir_acceptable(&missing).is_err());
        std::fs::write(tmp.path().join("file.txt"), "x").unwrap();
        assert!(dir_acceptable(&tmp.path().join("file.txt")).is_err());
    }

    #[test]
    fn path_merge_dedupes_case_insensitively() {
        assert!(path_contains(r"C:\a;C:\scoop\shims;C:\b", Path::new(r"C:\SCOOP\SHIMS")));
        let merged = merge_path_var(r"C:\a;C:\b", Path::new(r"C:\scoop\shims"));
        assert_eq!(merged, r"C:\scoop\shims;C:\a;C:\b");
        let unchanged = merge_path_var(r"C:\x;C:\scoop\shims", Path::new(r"C:\scoop\shims"));
        assert_eq!(unchanged, r"C:\x;C:\scoop\shims");
        assert_eq!(merge_path_var("", Path::new(r"C:\scoop\shims")), r"C:\scoop\shims");
    }

    #[test]
    fn extract_command_prefers_tar_and_falls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        std::fs::write(&zip, "z").unwrap();
        let staging = tmp.path().join("out");
        let (cmd, script) = extract_command(&zip, &staging, true, tmp.path()).unwrap();
        assert!(cmd.program.to_lowercase().ends_with("tar.exe"));
        assert!(script.is_none());
        let (cmd2, script2) = extract_command(&zip, &staging, false, tmp.path()).unwrap();
        assert_eq!(cmd2.program, "powershell.exe");
        let script2 = script2.unwrap();
        let text = std::fs::read_to_string(&script2).unwrap();
        assert!(text.contains("Expand-Archive"));
        assert!(text.contains("b.zip"));
    }

    #[test]
    fn finalize_moves_and_scaffolds() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scoop");
        let staging = staging_dir(&dir);
        std::fs::create_dir_all(staging.join("apps").join("scoop").join("current").join("bin"))
            .unwrap();
        std::fs::write(
            staging.join("apps").join("scoop").join("current").join("bin").join("scoop.ps1"),
            "#",
        )
        .unwrap();
        std::fs::create_dir_all(staging.join("buckets").join("main").join("bucket")).unwrap();
        std::fs::write(
            staging.join("buckets").join("main").join("bucket").join("git.json"),
            "{}",
        )
        .unwrap();
        std::fs::write(staging.join("bundle.json"), "{}").unwrap();
        let count = finalize(&dir, &staging).unwrap();
        assert_eq!(count, 1);
        assert!(dir
            .join("apps")
            .join("scoop")
            .join("current")
            .join("bin")
            .join("scoop.ps1")
            .is_file());
        assert!(dir.join("buckets").join("main").join("bucket").join("git.json").is_file());
        for sub in ["cache", "persist", "shims"] {
            assert!(dir.join(sub).is_dir());
        }
        assert!(!staging.exists());
    }

    #[test]
    fn finalize_rejects_incomplete_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scoop");
        let staging = staging_dir(&dir);
        std::fs::create_dir_all(staging.join("apps")).unwrap();
        assert!(finalize(&dir, &staging).is_err());
    }

    #[test]
    fn shims_written_with_relative_launcher() {
        let tmp = tempfile::tempdir().unwrap();
        ensure_shims(tmp.path()).unwrap();
        let ps1 = std::fs::read_to_string(tmp.path().join("shims").join("scoop.ps1")).unwrap();
        let cmd = std::fs::read_to_string(tmp.path().join("shims").join("scoop.cmd")).unwrap();
        assert!(ps1.contains(r"..\apps\scoop\current\bin\scoop.ps1"));
        assert!(cmd.contains(r"..\apps\scoop\current\bin\scoop.ps1"));
    }

    #[test]
    fn bundle_info_parses_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("scoop-bundle.json");
        std::fs::write(
            &path,
            r#"{"built":"2026-10-13 10:00:00","scoop":{"repo":"r1","commit":"abc"},"main":{"repo":"r2","commit":"def"}}"#,
        )
        .unwrap();
        let info = read_bundle_info(&path).unwrap();
        assert_eq!(info.scoop_commit, "abc");
        assert_eq!(info.main_commit, "def");
        assert!(read_bundle_info(&tmp.path().join("nope.json")).is_err());
    }

    #[test]
    fn staging_is_sibling_of_target() {
        assert_eq!(
            staging_dir(Path::new(r"C:\scoop")),
            PathBuf::from(r"C:\scoop.scoopui-extract")
        );
    }

    #[test]
    fn finalize_rolls_back_on_incomplete_content() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scoop");
        let staging = staging_dir(&dir);
        std::fs::create_dir_all(staging.join("apps").join("scoop").join("current").join("bin"))
            .unwrap();
        std::fs::write(
            staging.join("apps").join("scoop").join("current").join("bin").join("scoop.ps1"),
            "#",
        )
        .unwrap();
        std::fs::create_dir_all(staging.join("buckets").join("main").join("bucket")).unwrap();
        assert!(finalize(&dir, &staging).is_err());
        assert!(!dir.join("apps").exists());
        assert!(!dir.join("buckets").exists());
        assert!(!staging.exists());
    }

    #[test]
    fn installer_text_validation() {
        let filler = "x".repeat(1200);
        assert!(validate_installer_text(&format!("{filler}$ScoopDir Install-Scoop")).is_ok());
        assert!(validate_installer_text(&format!("{filler}Install-Scoop")).is_err());
        assert!(validate_installer_text(&format!("{filler}$ScoopDir")).is_err());
        assert!(validate_installer_text("$ScoopDir Install-Scoop").is_err());
    }

    #[test]
    fn staging_marker_guards_foreign_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        std::fs::write(&zip, "z").unwrap();
        let staging = tmp.path().join("out.scoopui-extract");
        extract_command(&zip, &staging, true, tmp.path()).unwrap();
        assert!(staging.join(".scoopui-staging").is_file());
        extract_command(&zip, &staging, true, tmp.path()).unwrap();
        std::fs::remove_file(staging.join(".scoopui-staging")).unwrap();
        std::fs::write(staging.join("user-data.txt"), "keep").unwrap();
        assert!(extract_command(&zip, &staging, true, tmp.path()).is_err());
        assert!(staging.join("user-data.txt").is_file());
    }

    #[test]
    fn assemble_report_flags_dir_problems() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("bad");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("f"), "x").unwrap();
        let report = assemble_report(
            &bad,
            Path::new(r"C:\Users\u\scoop"),
            Some(PathBuf::from(r"C:\scoop")),
            ProbeResults {
                powershell_major: Some(5),
                tar_available: true,
                git_available: false,
                elevated: true,
            },
        );
        assert!(!report.dir_ok);
        assert!(report.dir_reason.is_some());
        assert!(report.powershell_ok);
        assert!(!report.git_available);
        assert!(report.elevated);
        assert_eq!(report.existing_root.as_deref(), Some(r"C:\scoop"));
    }
}
