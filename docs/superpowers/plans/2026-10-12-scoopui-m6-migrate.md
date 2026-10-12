# ScoopUI M6 环境迁移 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 扫描 Windows 注册表中已安装的非 scoop 软件，按置信度匹配本地清单索引，引导用户用 scoop 接管安装，可选先卸载旧版（默认关）。

**Architecture:** 后端：索引 `PackageEntry` 增加 `bin`；新增 `domain/registry.rs`（windows-sys 枚举/读取注册表 + 纯解析过滤）与 `domain/migrate.rs`（纯匹配评分）；`runner` 抽取通用 `spawn_command` 供代卸载复用；新增 3 个命令（`scan_installed_registry` / `match_registry_apps` / `run_registry_uninstall`）。前端：`stores/migrate.ts` + `pages/MigratePage.vue`；`ui` store 增加 `storeSearchQuery` 供「查看」跳转商店搜索。

**Tech Stack:** Rust（Tauri 2、windows-sys 既有依赖新增 feature `Win32_System_Registry`）、Vue 3 + TypeScript + Naive UI + Pinia + Vitest。无新增 crate。

**Spec:** `docs/superpowers/specs/2026-10-12-scoopui-m5b-m7-remaining-design.md` §3

## Global Constraints

- 无新增依赖 crate；`windows-sys` 仅新增 feature（允许）
- 契约加法：仅新增字段/命令；不改既有命令参数/返回结构
- 代码不加注释；不加依赖；UI 文案全中文（术语用「软件源」）
- 代卸载安全：`run_registry_uninstall` 只接受三处 Uninstall 根下的注册表键路径，后端凭 key 重新读取 UninstallString，不接受前端任意命令串
- 失败不阻断：代卸载失败仅提示，继续安装
- 验证门：`cargo test` 全绿；`pnpm test` 全绿；`pnpm build`（vue-tsc）无错
- 初始 BASE 提交：`d16b594`
- 当前基线：Rust 130 passed；前端 152 passed（23 文件）
- Windows 环境：Rust 命令前先 `export PATH="$USERPROFILE/.cargo/bin:$PATH"`

## File Structure

**后端**
- `src-tauri/Cargo.toml`（改）：windows-sys features 加 `Win32_System_Registry`
- `src-tauri/src/domain/index.rs`（改）：`PackageEntry.bin`
- `src-tauri/src/domain/registry.rs`（新）：枚举/读取/解析/过滤/exe 名收集/卸载命令读取
- `src-tauri/src/domain/migrate.rs`（新）：匹配评分
- `src-tauri/src/domain/mod.rs`（改）：注册两个模块
- `src-tauri/src/commands/runner.rs`（改）：抽取 `spawn_command`
- `src-tauri/src/commands/migrate.rs`（新）：3 个命令
- `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`（改）：注册

**前端**
- `src/api/types.ts`、`src/api/commands.ts`（改）：迁移类型与命令封装
- `src/stores/ui.ts`（改）：`storeSearchQuery` + `searchInStore`
- `src/pages/StorePage.vue`（改）：挂载时消费 `storeSearchQuery`
- `src/stores/migrate.ts`（新）：扫描/安装编排
- `src/pages/MigratePage.vue`（新）：迁移页
- `src/components/layout/AppShell.vue`（改）：`migrate` 路由

**测试**
- 后端：`domain/index.rs`、`domain/registry.rs`、`domain/migrate.rs` 内联测试；`updates.rs`/`search.rs` 的 `PackageEntry` fixture 同步补 `bin`
- 前端：`src/__tests__/migrate.spec.ts`、`migrate-page.spec.ts`（新）；`commands.spec.ts`、`appshell.spec.ts`、`store-page.spec.ts`（改）

---

### Task 1: 索引 bin 扩展

**Files:**
- Modify: `src-tauri/src/domain/index.rs`
- Modify: `src-tauri/src/domain/updates.rs`（测试 fixture）
- Modify: `src-tauri/src/commands/search.rs`（测试 fixture）

**Interfaces:**
- Produces: `PackageEntry.bin: Vec<String>`（来自 manifest `bin` 字段，已去重顺序保留）

- [ ] **Step 1: 写失败测试**

`src-tauri/src/domain/index.rs` 的 `mod tests` 追加：

```rust
    #[test]
    fn collects_bin_from_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(
            tmp.path(),
            "main",
            "git",
            r#"{"version":"1.0","bin":[["git","git.exe"],"scalar.exe"]}"#,
        );
        let entries = build(tmp.path());
        assert_eq!(entries[0].bin, vec!["git", "scalar.exe"]);
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib index::`
Expected: 编译失败（`PackageEntry` 无 `bin` 字段）

- [ ] **Step 3: 实现**

`src-tauri/src/domain/index.rs`：`PackageEntry` 在 `icon` 之后加字段并在 `build` 中赋值：

```rust
    pub icon: Option<String>,
    pub bin: Vec<String>,
    pub deprecated: bool,
```

```rust
                icon: m.icon,
                bin: m.bin,
                deprecated: m.deprecated,
```

同步补齐所有 `PackageEntry` 字面量 fixture（编译器会逐一指出）：
- `src-tauri/src/domain/index.rs` 测试内 3 个 `mk` 闭包：加 `bin: vec![],`
- `src-tauri/src/domain/updates.rs` 测试 helper `entry()`：加 `bin: vec![],`
- `src-tauri/src/commands/search.rs` 测试 helper `entry()`：加 `bin: vec![],`

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 131 passed（130 + 1）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/index.rs src-tauri/src/domain/updates.rs src-tauri/src/commands/search.rs
git commit -m "feat: 索引扩展 bin 字段"
```

---

### Task 2: 注册表领域（扫描/解析/过滤/代卸载读取）

**Files:**
- Modify: `src-tauri/Cargo.toml`（features 加 `Win32_System_Registry`）
- Create: `src-tauri/src/domain/registry.rs`
- Modify: `src-tauri/src/domain/mod.rs`

**Interfaces:**
- Consumes: `root::normalized`
- Produces: `UNINSTALL_ROOTS: [&str; 3]`、`RegistryApp`、`RawEntry`、`parse_entry(&RawEntry, &[PathBuf]) -> Option<RegistryApp>`、`collect_exe_names(&Path) -> Vec<String>`、`is_under_roots(&Path, &[PathBuf]) -> bool`、`scan_registry(&[PathBuf]) -> Vec<RegistryApp>`、`read_uninstall_string(&str) -> Result<String, AppError>`

- [ ] **Step 1: 写失败测试**

新建 `src-tauri/src/domain/registry.rs`（先放类型与测试；实现由 Step 3 补全；`domain/mod.rs` 注册 `pub mod registry;`，编译红）：

```rust
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
    #[ignore]
    fn scans_real_registry() {
        let apps = scan_registry(&[]);
        assert!(!apps.is_empty());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib registry::`
Expected: 编译失败（类型与函数未定义）

- [ ] **Step 3: 实现**

`src-tauri/Cargo.toml`：

```toml
windows-sys = { version = "0.61.2", features = ["Win32_Foundation", "Win32_Security", "Win32_System_JobObjects", "Win32_System_Registry"] }
```

`src-tauri/src/domain/registry.rs` 在测试模块之前：

```rust
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
    let normalized = crate::domain::root::normalized(path);
    roots
        .iter()
        .any(|r| normalized.starts_with(&crate::domain::root::normalized(r)))
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
    UNINSTALL_ROOTS
        .iter()
        .any(|root| key.len() > root.len() && key[..root.len()].eq_ignore_ascii_case(root) && key.as_bytes()[root.len()] == b'\\')
}
```

Windows 专用实现（放在 `valid_key_path` 之后；非 Windows 空实现）：

```rust
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
```

注意：若 windows-sys 0.61 的绑定签名（`HKEY` 类型、`RegEnumKeyExW` 参数、常量名）与上面代码有出入，按编译器报错做最小修正，保持行为与测试断言不变（这是允许的编译适配）。`uninstall_string` 的 hive 前缀切分假定 key 以 `HKCU\` 或 `HKLM\` 开头，二者长度均为 5；请保持 `valid_key_path` 先校验。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 136 passed（131 + 5，另有 1 个 `#[ignore]` 真实扫描用例不计入）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/src/domain/registry.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 注册表扫描与解析（迁移用）"
```

---

### Task 3: 匹配评分领域

**Files:**
- Create: `src-tauri/src/domain/migrate.rs`
- Modify: `src-tauri/src/domain/mod.rs`

**Interfaces:**
- Consumes: `index::PackageEntry`（含 `bin`）、`registry::RegistryApp`
- Produces: `MigrationMatch`、`MigrationEntry`、`normalize_name(&str) -> String`、`match_apps(&[RegistryApp], &[PackageEntry]) -> Vec<MigrationEntry>`

- [ ] **Step 1: 写失败测试**

新建 `src-tauri/src/domain/migrate.rs`（先放测试；实现由 Step 3 补全；`domain/mod.rs` 注册 `pub mod migrate;`，编译红）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::index::PackageEntry;
    use crate::domain::registry::RegistryApp;

    fn raw_app(name: &str, exe: &[&str], managed: bool) -> RegistryApp {
        RegistryApp {
            key_path: r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\X".to_string(),
            name: name.to_string(),
            version: Some("1.0".to_string()),
            publisher: None,
            install_location: None,
            uninstall_string: None,
            exe_names: exe.iter().map(|s| s.to_string()).collect(),
            managed,
        }
    }

    fn entry(name: &str, desc: Option<&str>, bin: &[&str]) -> PackageEntry {
        PackageEntry {
            name: name.to_string(),
            bucket: "main".to_string(),
            version: "1".to_string(),
            description: desc.map(|s| s.to_string()),
            homepage: None,
            license: None,
            icon: None,
            bin: bin.iter().map(|s| s.to_string()).collect(),
            deprecated: false,
        }
    }

    #[test]
    fn normalize_strips_punctuation_and_case() {
        assert_eq!(normalize_name("Visual-Studio  Code!"), "visualstudiocode");
        assert_eq!(normalize_name("7-Zip"), "7zip");
    }

    #[test]
    fn high_confidence_on_normalized_exact_name() {
        let apps = vec![raw_app("Git", &[], false)];
        let entries = vec![entry("git", Some("Distributed VCS"), &["git"])];
        let out = match_apps(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].matches.len(), 1);
        assert_eq!(out[0].matches[0].confidence, "high");
        assert_eq!(out[0].matches[0].bucket, "main");
    }

    #[test]
    fn medium_confidence_on_bin_match() {
        let apps = vec![raw_app("Git for Windows", &["git"], false)];
        let entries = vec![entry("git", Some("distributed version control"), &["git.exe"])];
        let out = match_apps(&apps, &entries);
        assert_eq!(out[0].matches[0].confidence, "medium");
    }

    #[test]
    fn low_confidence_on_description_word_overlap() {
        let apps = vec![raw_app("Adobe Acrobat Reader DC", &[], false)];
        let entries = vec![entry(
            "pdf-reader",
            Some("Read and annotate Adobe Acrobat documents"),
            &[],
        )];
        let out = match_apps(&apps, &entries);
        assert_eq!(out[0].matches.len(), 1);
        assert_eq!(out[0].matches[0].confidence, "low");
    }

    #[test]
    fn no_match_and_managed_passthrough() {
        let apps = vec![raw_app("Totally Unrelated", &[], true)];
        let entries = vec![entry("git", None, &[])];
        let out = match_apps(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert!(out[0].matches.is_empty());
        assert!(out[0].app.managed);
    }

    #[test]
    fn sorts_by_confidence_then_name_and_limits() {
        let apps = vec![raw_app("Git Tools Suite", &["scalar"], false)];
        let entries = vec![
            entry("gittoolssuite", Some("all-in-one bundle"), &[]),
            entry("git-lfs", Some("git large file storage"), &["scalar.exe"]),
            entry("git-tools", Some("suite of git tools"), &[]),
        ];
        let out = match_apps(&apps, &entries);
        let confidences: Vec<&str> = out[0].matches.iter().map(|m| m.confidence.as_str()).collect();
        assert_eq!(confidences, vec!["high", "medium", "low"]);
        let names: Vec<&str> = out[0].matches.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["gittoolssuite", "git-lfs", "git-tools"]);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib migrate::`
Expected: 编译失败（类型与函数未定义）

- [ ] **Step 3: 实现**

`src-tauri/src/domain/migrate.rs` 在测试模块之前：

```rust
use serde::Serialize;

use crate::domain::index::PackageEntry;
use crate::domain::registry::RegistryApp;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MigrationMatch {
    pub name: String,
    pub bucket: String,
    pub confidence: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MigrationEntry {
    pub app: RegistryApp,
    pub matches: Vec<MigrationMatch>,
}

pub fn normalize_name(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_lowercase())
        .collect()
}

fn bin_stems(entry: &PackageEntry) -> Vec<String> {
    entry
        .bin
        .iter()
        .map(|b| {
            let file = b.rsplit(['\\', '/']).next().unwrap_or(b);
            file.trim_end_matches(".exe")
                .trim_end_matches(".EXE")
                .to_lowercase()
        })
        .collect()
}

fn rank(confidence: &str) -> u8 {
    match confidence {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    }
}

pub fn match_apps(apps: &[RegistryApp], entries: &[PackageEntry]) -> Vec<MigrationEntry> {
    let mut out: Vec<MigrationEntry> = apps
        .iter()
        .map(|app| {
            let normalized = normalize_name(&app.name);
            let name_words = words(&app.name);
            let mut matches: Vec<MigrationMatch> = Vec::new();
            for entry in entries {
                let confidence = if !normalized.is_empty() && normalize_name(&entry.name) == normalized {
                    "high"
                } else if app.exe_names.iter().any(|e| bin_stems(entry).iter().any(|b| b == e)) {
                    "medium"
                } else {
                    let desc_words = entry.description.as_deref().map(words).unwrap_or_default();
                    let overlap = name_words.iter().filter(|w| desc_words.contains(w)).count();
                    if overlap >= 2 {
                        "low"
                    } else {
                        continue;
                    }
                };
                matches.push(MigrationMatch {
                    name: entry.name.clone(),
                    bucket: entry.bucket.clone(),
                    confidence: confidence.to_string(),
                    version: entry.version.clone(),
                    description: entry.description.clone(),
                });
            }
            matches.sort_by(|a, b| {
                rank(&a.confidence)
                    .cmp(&rank(&b.confidence))
                    .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            matches.truncate(5);
            MigrationEntry {
                app: app.clone(),
                matches,
            }
        })
        .collect();
    out.sort_by(|a, b| a.app.name.to_lowercase().cmp(&b.app.name.to_lowercase()));
    out
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 142 passed（136 + 6）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/migrate.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 迁移匹配评分"
```

---

### Task 4: 命令层（runner 抽取 + 3 命令 + 注册）

**Files:**
- Modify: `src-tauri/src/commands/runner.rs`
- Create: `src-tauri/src/commands/migrate.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 2/3 的 domain 函数；`search::load_index`；`scoopcmd::ScoopCommand`
- Produces: `runner::spawn_command(app, &AppState, ScoopCommand, task_id)`；命令 `scan_installed_registry` / `match_registry_apps` / `run_registry_uninstall`

- [ ] **Step 1: 实现**（runner 抽取为等价重构，domain 逻辑已由 Task 2/3 覆盖；验证方式为编译 + 全量测试）

`src-tauri/src/commands/runner.rs`：把 `spawn_task` 拆成通用 `spawn_command` + scoop 组装：

```rust
pub(crate) fn spawn_command(
    app: &AppHandle,
    state: &AppState,
    cmd: scoopcmd::ScoopCommand,
    task_id: String,
) -> Result<(), AppError> {
    let spawned = {
        let mut tasks = state.tasks.lock().unwrap();
        ensure_idle(&tasks)?;
        let spawned = scoopcmd::spawn(&cmd)?;
        tasks.insert(task_id.clone(), spawned.pid());
        spawned
    };

    let app_handle = app.clone();
    let task = task_id.clone();
    std::thread::spawn(move || {
        let emit_handle = app_handle.clone();
        let emit_task = task.clone();
        let result = scoopcmd::stream_child(spawned, move |l| {
            let _ = emit_handle.emit(
                "scoop-log",
                LogPayload {
                    task_id: emit_task.clone(),
                    stream: l.stream.to_string(),
                    line: l.line,
                },
            );
        });
        let (code, success) = match result {
            Ok(c) => (c, c == Some(0)),
            Err(_) => (None, false),
        };
        let _ = app_handle.emit(
            "scoop-task-end",
            EndPayload { task_id: task.clone(), code, success },
        );
        if let Some(state) = app_handle.try_state::<AppState>() {
            state.tasks.lock().unwrap().remove(&task);
        }
    });
    Ok(())
}

pub(crate) fn spawn_task(
    app: &AppHandle,
    state: &AppState,
    args: Vec<String>,
    task_id: String,
) -> Result<(), AppError> {
    let ctx = context();
    let root_path = resolve_active_root(&ctx, state).ok_or(AppError::NoRoot)?;
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    let cmd = scoopcmd::build(&root_path, Some(&global), &args)?;
    spawn_command(app, state, cmd, task_id)
}
```

新建 `src-tauri/src/commands/migrate.rs`：

```rust
use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::commands::runner::spawn_command;
use crate::commands::search::load_index;
use crate::commands::{context, roots::resolve_active_root};
use crate::domain::migrate::{self, MigrationEntry};
use crate::domain::registry::{self, RegistryApp};
use crate::domain::scoopcmd::ScoopCommand;
use crate::error::AppError;
use crate::settings;
use crate::state::AppState;

fn known_roots(ctx: &crate::commands::Context, active: Option<&PathBuf>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = settings::load(&ctx.settings_dir())
        .known_roots
        .into_iter()
        .map(PathBuf::from)
        .collect();
    if let Some(active) = active {
        roots.push(active.clone());
    }
    roots
}

#[tauri::command]
pub async fn scan_installed_registry(state: State<'_, AppState>) -> Result<Vec<RegistryApp>, AppError> {
    let ctx = context();
    let active = resolve_active_root(&ctx, &state);
    let roots = known_roots(&ctx, active.as_ref());
    let apps = tauri::async_runtime::spawn_blocking(move || registry::scan_registry(&roots))
        .await
        .map_err(|e| AppError::Other(format!("注册表扫描失败: {e}")))?;
    Ok(apps)
}

#[tauri::command]
pub async fn match_registry_apps(state: State<'_, AppState>) -> Result<Vec<MigrationEntry>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = load_index(&state, &root).await?;
    let roots = known_roots(&ctx, Some(&root));
    let apps = tauri::async_runtime::spawn_blocking(move || registry::scan_registry(&roots))
        .await
        .map_err(|e| AppError::Other(format!("注册表扫描失败: {e}")))?;
    Ok(migrate::match_apps(&apps, &entries))
}

#[tauri::command(async)]
pub fn run_registry_uninstall(
    app: AppHandle,
    state: State<'_, AppState>,
    entry_key: String,
    task_id: String,
) -> Result<(), AppError> {
    let uninstall = registry::read_uninstall_string(&entry_key)?;
    let cmd = ScoopCommand {
        program: "cmd.exe".to_string(),
        args: vec!["/C".to_string(), uninstall],
        envs: vec![],
        display: format!("uninstall {entry_key}"),
    };
    spawn_command(&app, &state, cmd, task_id)
}
```

`src-tauri/src/commands/mod.rs` 的 `pub mod` 列表插入（字母序，`icons` 之后）：

```rust
pub mod migrate;
```

`src-tauri/src/lib.rs` 的 `invoke_handler` 中 `commands::snapshots::restore_snapshot_buckets` 之后追加（注意前一行为逗号）：

```rust
            commands::migrate::scan_installed_registry,
            commands::migrate::match_registry_apps,
            commands::migrate::run_registry_uninstall
```

- [ ] **Step 2: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 142 passed（无新增测试；runner 既有 6 个测试（含 cancel 真实进程用例）验证抽取重构无回归）

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/commands/runner.rs src-tauri/src/commands/migrate.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: 迁移命令层与 runner 抽取"
```

---

### Task 5: 前端 API、ui 搜索意图与 migrate store

**Files:**
- Modify: `src/api/types.ts`
- Modify: `src/api/commands.ts`
- Modify: `src/stores/ui.ts`
- Modify: `src/pages/StorePage.vue`
- Create: `src/stores/migrate.ts`
- Create: `src/__tests__/migrate.spec.ts`
- Modify: `src/__tests__/commands.spec.ts`
- Modify: `src/__tests__/store-page.spec.ts`

**Interfaces:**
- Consumes: Task 4 的 3 个命令；`runTask`
- Produces: `RegistryApp` / `MigrationMatch` / `MigrationEntry` 类型；`scanInstalledRegistry` / `matchRegistryApps` / `runRegistryUninstall` 封装；`ui.searchInStore(query)` 与 `ui.storeSearchQuery`；`useMigrateStore`（scan/install/visible/managedCount/matchedCount）

- [ ] **Step 1: 写失败测试**

`src/__tests__/commands.spec.ts`：`import` 列表补 `matchRegistryApps, runRegistryUninstall, scanInstalledRegistry`，并追加：

```ts
  it("M6 迁移命令名与参数符合后端契约", async () => {
    await scanInstalledRegistry();
    expect(invokeMock).toHaveBeenLastCalledWith("scan_installed_registry", undefined);

    await matchRegistryApps();
    expect(invokeMock).toHaveBeenLastCalledWith("match_registry_apps", undefined);

    await runRegistryUninstall("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\X", "t-1");
    expect(invokeMock).toHaveBeenLastCalledWith("run_registry_uninstall", {
      entryKey: "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\X",
      taskId: "t-1",
    });
  });
```

新建 `src/__tests__/migrate.spec.ts`：

```ts
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  matchRegistryApps: vi.fn(),
  runRegistryUninstall: vi.fn(),
  installPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { useLogsStore } from "../stores/logs";
import { useMigrateStore } from "../stores/migrate";
import type { MigrationEntry } from "../api/types";

function entry(name: string, managed: boolean): MigrationEntry {
  return {
    app: {
      key_path: `HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${name}`,
      name,
      version: "1.0",
      publisher: null,
      install_location: null,
      uninstall_string: null,
      exe_names: [],
      managed,
    },
    matches: [
      { name: name.toLowerCase(), bucket: "main", confidence: "high", version: "2.0", description: null },
    ],
  };
}

describe("migrate store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("scan 填充条目并过滤已管理", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false), entry("ScoopManaged", true)]);
    const store = useMigrateStore();
    await store.scan();
    expect(store.scanned).toBe(true);
    expect(store.entries).toHaveLength(2);
    expect(store.visible).toHaveLength(1);
    expect(store.managedCount).toBe(1);
    store.showManaged = true;
    expect(store.visible).toHaveLength(2);
  });

  it("scan 失败时 error 可读", async () => {
    mocks.matchRegistryApps.mockRejectedValue("未找到可用的 scoop 根目录");
    const store = useMigrateStore();
    await store.scan();
    expect(store.entries).toEqual([]);
    expect(store.error).toContain("未找到");
  });

  it("install 勾选时先卸载再安装", async () => {
    mocks.runRegistryUninstall.mockResolvedValue(undefined);
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useMigrateStore();
    const e = entry("App", false);
    const running = store.install(e, e.matches[0], true);
    await flushPromises();
    expect(mocks.runRegistryUninstall).toHaveBeenCalledWith(e.app.key_path, expect.any(String));
    const uninstallTask = mocks.runRegistryUninstall.mock.calls[0][1];
    useLogsStore().end({ taskId: uninstallTask, code: 0, success: true });
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("app", "main", null, false, expect.any(String));
    const installTask = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId: installTask, code: 0, success: true });
    await flushPromises();
    const result = await running;
    expect(result.ok).toBe(true);
    expect(result.warning).toBeNull();
    expect(mocks.listInstalledApps).toHaveBeenCalled();
  });

  it("卸载失败仅警告并继续安装", async () => {
    mocks.runRegistryUninstall.mockResolvedValue(undefined);
    mocks.installPackage.mockResolvedValue(undefined);
    const store = useMigrateStore();
    const e = entry("App", false);
    const running = store.install(e, e.matches[0], true);
    await flushPromises();
    const uninstallTask = mocks.runRegistryUninstall.mock.calls[0][1];
    useLogsStore().end({ taskId: uninstallTask, code: 1, success: false });
    await flushPromises();
    const installTask = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId: installTask, code: 0, success: true });
    await flushPromises();
    const result = await running;
    expect(result.ok).toBe(true);
    expect(result.warning).toContain("旧版卸载失败");
  });
});
```

`src/__tests__/store-page.spec.ts`：`import` 处补 `useUiStore`，并在 `describe("StorePage")` 内追加：

```ts
  it("消费来自迁移页的搜索意图", async () => {
    useUiStore().storeSearchQuery = "7zip";
    mount(StorePage);
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledWith("7zip", 50);
    expect(useUiStore().storeSearchQuery).toBeNull();
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/migrate.spec.ts src/__tests__/commands.spec.ts src/__tests__/store-page.spec.ts`
Expected: FAIL（`stores/migrate.ts`、命令封装与 `storeSearchQuery` 不存在）

- [ ] **Step 3: 实现**

`src/api/types.ts` 末尾追加：

```ts
export interface RegistryApp {
  key_path: string;
  name: string;
  version: string | null;
  publisher: string | null;
  install_location: string | null;
  uninstall_string: string | null;
  exe_names: string[];
  managed: boolean;
}

export interface MigrationMatch {
  name: string;
  bucket: string;
  confidence: string;
  version: string;
  description: string | null;
}

export interface MigrationEntry {
  app: RegistryApp;
  matches: MigrationMatch[];
}
```

`src/api/commands.ts` 末尾追加（`import type` 列表补 `MigrationEntry, RegistryApp`）：

```ts
export const scanInstalledRegistry = () => invoke<RegistryApp[]>("scan_installed_registry");
export const matchRegistryApps = () => invoke<MigrationEntry[]>("match_registry_apps");
export const runRegistryUninstall = (entryKey: string, taskId: string) =>
  invoke<void>("run_registry_uninstall", { entryKey, taskId });
```

`src/stores/ui.ts`：state 增加 `storeSearchQuery`，actions 增加 `searchInStore`：

```ts
  state: () => ({
    activePage: "installed" as PageId,
    logDrawerOpen: false,
    storeSearchQuery: null as string | null,
  }),
```

```ts
    searchInStore(query: string) {
      this.storeSearchQuery = query;
      this.activePage = "store";
    },
```

`src/pages/StorePage.vue`：`script setup` 中 `import { useUiStore } from "../stores/ui";`，`onMounted` 改为：

```ts
onMounted(() => {
  const ui = useUiStore();
  const pending = ui.storeSearchQuery;
  if (pending !== null) {
    input.value = pending;
    ui.storeSearchQuery = null;
    pkgs.search(pending);
    return;
  }
  pkgs.search(pkgs.query);
});
```

新建 `src/stores/migrate.ts`：

```ts
import { defineStore } from "pinia";
import { installPackage, matchRegistryApps, runRegistryUninstall } from "../api/commands";
import type { MigrationEntry, MigrationMatch } from "../api/types";
import { runTask } from "../utils/tasks";
import { useAppsStore } from "./apps";
import { useUiStore } from "./ui";

export interface InstallResult {
  ok: boolean;
  warning: string | null;
  error: string | null;
}

export const useMigrateStore = defineStore("migrate", {
  state: () => ({
    entries: [] as MigrationEntry[],
    scanned: false,
    loading: false,
    error: null as string | null,
    showManaged: false,
  }),
  getters: {
    visible(state): MigrationEntry[] {
      return state.showManaged ? state.entries : state.entries.filter((e) => !e.app.managed);
    },
    managedCount(state): number {
      return state.entries.filter((e) => e.app.managed).length;
    },
    matchedCount(state): number {
      return state.entries.filter((e) => !e.app.managed && e.matches.length > 0).length;
    },
  },
  actions: {
    async scan() {
      this.loading = true;
      this.error = null;
      try {
        this.entries = await matchRegistryApps();
        this.scanned = true;
      } catch (e) {
        this.entries = [];
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    async install(
      entry: MigrationEntry,
      match: MigrationMatch,
      uninstallFirst: boolean,
    ): Promise<InstallResult> {
      useUiStore().logDrawerOpen = true;
      let warning: string | null = null;
      if (uninstallFirst) {
        const uninstall = await runTask(`卸载旧版 ${entry.app.name}`, (taskId) =>
          runRegistryUninstall(entry.app.key_path, taskId),
        );
        if (!uninstall.success) {
          warning = `旧版卸载失败（退出码 ${uninstall.code ?? "未知"}），已继续安装`;
        }
      }
      const record = await runTask(`安装 ${match.name}`, (taskId) =>
        installPackage(match.name, match.bucket, null, false, taskId),
      );
      if (record.success) {
        await useAppsStore().load();
        return { ok: true, warning, error: null };
      }
      return { ok: false, warning, error: `步骤失败（退出码 ${record.code ?? "未知"}）` };
    },
  },
});
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/migrate.spec.ts src/__tests__/commands.spec.ts src/__tests__/store-page.spec.ts`
Expected: PASS（migrate 4 + commands 8 + store-page 15 + 1）

- [ ] **Step 5: 提交**

```bash
git add src/api/types.ts src/api/commands.ts src/stores/ui.ts src/pages/StorePage.vue src/stores/migrate.ts src/__tests__/migrate.spec.ts src/__tests__/commands.spec.ts src/__tests__/store-page.spec.ts
git commit -m "feat(ui): 迁移 API、搜索意图与 store"
```

---

### Task 6: 迁移页面与路由

**Files:**
- Create: `src/pages/MigratePage.vue`
- Modify: `src/components/layout/AppShell.vue`
- Create: `src/__tests__/migrate-page.spec.ts`
- Modify: `src/__tests__/appshell.spec.ts`

**Interfaces:**
- Consumes: Task 5 的 `useMigrateStore`、`ui.searchInStore`
- Produces: `migrate` 页路由；扫描/安装/查看交互

- [ ] **Step 1: 写失败测试**

新建 `src/__tests__/migrate-page.spec.ts`：

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const messageWarning = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  matchRegistryApps: vi.fn(),
  runRegistryUninstall: vi.fn(),
  installPackage: vi.fn(),
  listInstalledApps: vi.fn(),
  listUpdates: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: messageSuccess,
      warning: messageWarning,
      info: vi.fn(),
    }),
  };
});

import MigratePage from "../pages/MigratePage.vue";
import { useLogsStore } from "../stores/logs";
import { useMigrateStore } from "../stores/migrate";
import { useUiStore } from "../stores/ui";
import type { MigrationEntry } from "../api/types";

function entry(name: string, managed: boolean, confidence = "high"): MigrationEntry {
  return {
    app: {
      key_path: `HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${name}`,
      name,
      version: "1.0",
      publisher: "Pub",
      install_location: null,
      uninstall_string: null,
      exe_names: [],
      managed,
    },
    matches: [
      { name: name.toLowerCase(), bucket: "main", confidence, version: "2.0", description: null },
    ],
  };
}

describe("MigratePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([]);
    mocks.listUpdates.mockResolvedValue([]);
  });

  it("未扫描显示空态，扫描后渲染分组与匹配", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false)]);
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    expect(wrapper.text()).toContain("扫描本机已安装软件");
    const btn = wrapper.findAll("button").find((b) => b.text().includes("扫描"));
    await btn!.trigger("click");
    await flushPromises();
    expect(mocks.matchRegistryApps).toHaveBeenCalled();
    expect(wrapper.text()).toContain("App");
    expect(wrapper.text()).toContain("高置信匹配");
    expect(wrapper.text()).toContain("main/app");
  });

  it("安装调用 installPackage 并提示成功", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false)]);
    mocks.installPackage.mockResolvedValue(undefined);
    const pinia = createPinia();
    setActivePinia(pinia);
    await useMigrateStore().scan();
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    await flushPromises();
    const install = wrapper.findAll("button").find((b) => b.text() === "安装");
    await install!.trigger("click");
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("app", "main", null, false, expect.any(String));
    expect(messageSuccess).toHaveBeenCalled();
  });

  it("勾选先卸载旧版时先调用卸载命令", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false)]);
    mocks.runRegistryUninstall.mockResolvedValue(undefined);
    mocks.installPackage.mockResolvedValue(undefined);
    const pinia = createPinia();
    setActivePinia(pinia);
    await useMigrateStore().scan();
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    await flushPromises();
    const checkboxes = wrapper.findAll("input[type=checkbox]");
    await checkboxes[1].setValue(true);
    const install = wrapper.findAll("button").find((b) => b.text() === "安装");
    await install!.trigger("click");
    await flushPromises();
    expect(mocks.runRegistryUninstall).toHaveBeenCalled();
    const uninstallTask = mocks.runRegistryUninstall.mock.calls[0][1];
    useLogsStore().end({ taskId: uninstallTask, code: 0, success: true });
    await flushPromises();
    const installTask = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId: installTask, code: 0, success: true });
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalled();
  });

  it("查看跳转商店并携带搜索词", async () => {
    mocks.matchRegistryApps.mockResolvedValue([entry("App", false)]);
    const pinia = createPinia();
    setActivePinia(pinia);
    await useMigrateStore().scan();
    const wrapper = mount(MigratePage, { global: { plugins: [pinia] } });
    await flushPromises();
    const view = wrapper.findAll("button").find((b) => b.text() === "查看");
    await view!.trigger("click");
    expect(useUiStore().storeSearchQuery).toBe("app");
    expect(useUiStore().activePage).toBe("store");
  });
});
```

`src/__tests__/appshell.spec.ts`：`stubs` 加 `MigratePage: true,`，并追加：

```ts
  it("环境迁移页渲染 MigratePage 而非占位", () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useUiStore().setPage("migrate");
    const wrapper = mount(AppShell, { global: { plugins: [pinia], stubs } });
    expect(wrapper.find("migrate-page-stub").exists()).toBe(true);
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/migrate-page.spec.ts src/__tests__/appshell.spec.ts`
Expected: FAIL（`MigratePage.vue` 不存在；AppShell 仍渲染占位）

- [ ] **Step 3: 实现**

新建 `src/pages/MigratePage.vue`：

```vue
<template>
  <div class="page">
    <div class="toolbar">
      <n-button size="small" :loading="migrate.loading" @click="doScan">扫描本机已安装软件</n-button>
      <n-checkbox v-model:checked="migrate.showManaged">显示已管理条目</n-checkbox>
      <n-checkbox v-model:checked="uninstallFirst">安装前先卸载旧版</n-checkbox>
      <span v-if="migrate.scanned" class="count">
        共 {{ migrate.entries.length }} 条 · 已由 scoop 管理 {{ migrate.managedCount }} 条 · 可接管
        {{ migrate.matchedCount }} 条
      </span>
    </div>

    <n-alert v-if="migrate.error" type="error" size="small" class="alert">{{ migrate.error }}</n-alert>
    <div v-if="!migrate.scanned && !migrate.error" class="hint">点击「扫描本机已安装软件」开始</div>
    <div v-else-if="groups.length === 0" class="hint">没有可迁移的条目</div>
    <div v-else class="groups">
      <div v-for="g in groups" :key="g.key" class="group">
        <div class="group-title">{{ g.title }}（{{ g.entries.length }}）</div>
        <div v-for="e in g.entries" :key="e.app.key_path" class="row">
          <div class="info">
            <div class="name">
              {{ e.app.name }}
              <span v-if="e.app.managed" class="badge">已管理</span>
            </div>
            <div class="meta">
              <span v-if="e.app.version">{{ e.app.version }}</span>
              <span v-if="e.app.publisher"> · {{ e.app.publisher }}</span>
              <span v-if="e.app.install_location"> · {{ e.app.install_location }}</span>
            </div>
          </div>
          <div class="match">
            <template v-if="e.matches.length > 0">
              <n-tag size="small" type="success">{{ matchLabel(e.matches[0]) }}</n-tag>
              <span v-if="e.matches.length > 1" class="more">等 {{ e.matches.length }} 个</span>
            </template>
            <span v-else class="hint">未匹配到软件包</span>
          </div>
          <div class="actions">
            <n-button size="tiny" :disabled="busy || e.matches.length === 0" @click="doInstall(e)">安装</n-button>
            <n-button size="tiny" :disabled="e.matches.length === 0" @click="doView(e)">查看</n-button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NAlert, NButton, NCheckbox, NTag, useMessage } from "naive-ui";
import type { MigrationEntry, MigrationMatch } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useMigrateStore } from "../stores/migrate";
import { useUiStore } from "../stores/ui";

interface Group {
  key: string;
  title: string;
  entries: MigrationEntry[];
}

const migrate = useMigrateStore();
const logs = useLogsStore();
const message = useMessage();
const busy = computed(() => logs.busy);
const uninstallFirst = ref(false);

const groups = computed<Group[]>(() => {
  const result: Group[] = [
    { key: "high", title: "高置信匹配", entries: [] },
    { key: "medium", title: "中置信匹配", entries: [] },
    { key: "low", title: "低置信匹配", entries: [] },
    { key: "none", title: "未匹配", entries: [] },
  ];
  for (const e of migrate.visible) {
    const best = e.matches[0]?.confidence;
    const key = best === "high" || best === "medium" || best === "low" ? best : "none";
    result.find((g) => g.key === key)!.entries.push(e);
  }
  return result.filter((g) => g.entries.length > 0);
});

const confidenceLabel: Record<string, string> = {
  high: "高置信",
  medium: "中置信",
  low: "低置信",
};

function matchLabel(match: MigrationMatch): string {
  return `${match.bucket}/${match.name}（${confidenceLabel[match.confidence] ?? match.confidence}）`;
}

async function doScan() {
  if (busy.value) return;
  await migrate.scan();
}

async function doInstall(entry: MigrationEntry) {
  const match = entry.matches[0];
  if (!match || busy.value) return;
  const result = await migrate.install(entry, match, uninstallFirst.value);
  if (result.ok) {
    if (result.warning) {
      message.warning(result.warning);
    } else {
      message.success(`已安装 ${match.name}`);
    }
  } else {
    message.error(result.error ?? "安装失败，详见任务日志");
  }
}

function doView(entry: MigrationEntry) {
  const match = entry.matches[0];
  if (!match) return;
  useUiStore().searchInStore(match.name);
}
</script>

<style scoped>
.page {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.count {
  color: #888;
  font-size: 12px;
}
.alert {
  margin-bottom: 12px;
}
.hint {
  color: #999;
  font-size: 13px;
  text-align: center;
  padding: 40px 0;
}
.group {
  margin-bottom: 18px;
}
.group-title {
  font-size: 13px;
  color: #666;
  margin-bottom: 6px;
}
.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 10px;
  border-bottom: 1px solid #f5f5f7;
}
.info {
  flex: 1;
  min-width: 0;
}
.name {
  font-size: 14px;
}
.meta {
  font-size: 12px;
  color: #888;
  word-break: break-all;
}
.match {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}
.more {
  font-size: 12px;
  color: #888;
}
.actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
.badge {
  margin-left: 6px;
  font-size: 11px;
  color: #18a058;
  border: 1px solid #18a058;
  border-radius: 3px;
  padding: 0 4px;
}
</style>
```

`src/components/layout/AppShell.vue`：

1) 模板在 `MaintenancePage` 之后插入：

```vue
        <MigratePage v-else-if="ui.activePage === 'migrate'" />
```

2) `script setup` 导入：

```ts
import MigratePage from "../../pages/MigratePage.vue";
```

3) 删除 `placeholders` 常量与 `placeholderTitle` computed，占位页改为静态：

```vue
        <PlaceholderPage v-else title="开发中" />
```

（若 `computed` 不再被使用，从 `vue` 导入中移除；`PlaceholderPage` 保留作为兜底。）

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/migrate-page.spec.ts src/__tests__/appshell.spec.ts`
Expected: PASS（新增 4 + 1 个用例 + 既有用例全绿）

Run: `pnpm test`
Expected: 163 passed（152 + commands 1 + migrate 4 + store-page 1 + migrate-page 4 + appshell 1）

- [ ] **Step 5: 提交**

```bash
git add src/pages/MigratePage.vue src/components/layout/AppShell.vue src/__tests__/migrate-page.spec.ts src/__tests__/appshell.spec.ts
git commit -m "feat(ui): 环境迁移页面与路由"
```

---

### Task 7: 全量验证

**Files:** 无新增；只做验证与收尾

- [ ] **Step 1: 后端全量测试**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 142 passed、0 failed

可选（真实环境）：`cargo test --lib registry:: -- --ignored` 验证真实注册表扫描

- [ ] **Step 2: 前端全量测试与构建**

Run: `pnpm test`
Expected: 163 passed

Run: `pnpm build`
Expected: `vue-tsc` 无错误、vite build 成功

- [ ] **Step 3: 手工验收清单（用户在 Windows 本机执行 `pnpm tauri dev`）**

1. 环境迁移页：点击扫描 → 出现分组列表（高/中/低/未匹配），统计数字正确
2. 已在 scoop 管理的软件默认隐藏，勾选「显示已管理条目」可见并带「已管理」标记
3. 「查看」跳转商店并自动搜索匹配包
4. 「安装」：真实安装一个小应用成功；可选勾选「安装前先卸载旧版」验证先跑 UninstallString（失败时仅警告并继续）
5. 扫描中/安装中按钮禁用与日志抽屉输出正常

- [ ] **Step 4: 修复与提交（仅当验证发现问题）**

若发现问题：修复 → 回到 Step 1 重新验证 → 提交（`fix: ...`）。全部通过则无需额外提交。

---

## Self-Review 记录

- 规格覆盖：§3.1 扫描（Task 2）、§3.2 匹配与索引扩展（Task 1/3）、§3.3 代卸载（Task 2 读取 + Task 4 命令 + runner 抽取）、§3.4 迁移页（Task 5/6）、§5 测试策略（各任务内联 + Task 7）。
- 占位符扫描：无 TBD；所有步骤含完整代码与精确命令；windows-sys 绑定允许按编译器做最小适配（已在 Task 2 注明）。
- 类型一致性：`RegistryApp`/`RawEntry`/`MigrationMatch`/`MigrationEntry` 在 Task 2/3 定义，Task 4 命令签名、Task 5 前端类型与 store、Task 6 页面逐一对齐；`PackageEntry.bin` 在 Task 1 定义并被 Task 3 的 `bin_stems` 消费；`runRegistryUninstall(entryKey, taskId)` 前后端参数名（camelCase→snake_case）一致。
- 已知取舍：注册表真实枚举不做单元测试（`#[ignore]` 真机用例 + 手工验收）；匹配最多保留 5 条、UI 默认取第一条；代卸载以 `cmd /C` 执行 UninstallString（MSI/UAC 由系统处理）。


