# ScoopUI M1a（后端只读内核）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Rust 侧实现只读数据内核与命令执行框架：多根目录探测/校验/切换、配置解析（portable 优先）、已装应用扫描、桶扫描、scoop 命令流式执行与取消，全部通过 Tauri command 暴露给前端（M1b 消费）。

**Architecture:** `src-tauri/src/domain/` 放纯逻辑（可单测、不依赖 Tauri），`src-tauri/src/commands/` 放薄 Tauri 命令层（取上下文 → 调 domain → 返回/发事件）。设置持久化在 `%APPDATA%\scoopui\settings.json`。所有 scoop 命令通过「目标根自己的 `apps/scoop/current/bin/scoop.ps1` + 注入 `SCOOP=<root>`」执行（设计文档 §4.1/§4.2）。

**Tech Stack:** Rust（Tauri 2）、thiserror（错误）、tempfile（仅 dev-dependencies）、serde/serde_json（已有）。

## Global Constraints

- 平台 Windows；路径处理用 `PathBuf`，不硬编码分隔符；比较路径时大小写不敏感（Windows 语义）
- 所有面向用户的错误信息用中文（设计文档 §6）
- 代码不加注释（项目约定）；标识符用英文
- TDD：每个逻辑模块先写失败测试再实现；测试用 `tempfile` 建临时目录，不依赖真实 scoop 安装
- 验证门：`cargo test`（src-tauri 下）必须全绿；`cargo check` 无 error；不动前端（`pnpm build` 保持通过）
- 依赖纪律：新增运行时依赖仅 `thiserror`；`tempfile` 仅 dev-dependencies
- 提交信息中文，遵循 `feat:` / `test:` / `chore:` 前缀
- 已实测事实（实现必须按此）：已装应用数据源 = `apps/<name>/current/{manifest.json,install.json}`，`scoop` 自身无 install.json 会被自然跳过；`current` 是 junction，直接按路径读取即可；bucket 的 git 远程在 `<bucket>/.git/config` 的 `[remote "origin"] url` 行；`install.json` 字段为 `bucket/architecture/hold/updated` 且都可缺失；脚本路径 `<root>\apps\scoop\current\bin\scoop.ps1`
- 初始 BASE 提交：2513296

---

### Task 1: 错误类型与依赖（error.rs）

**Files:**
- Create: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/lib.rs`（加 `mod error;` 与 `pub use`）
- Modify: `src-tauri/Cargo.toml`（`thiserror` 运行时依赖；`tempfile` dev-dependency）

**Interfaces:**
- Consumes: 无
- Produces: `pub enum AppError { NoRoot, InvalidRoot(String), Io(String), Json(String), Other(String) }`，实现 `std::error::Error`、`From<std::io::Error>`、`From<serde_json::Error>`、`serde::Serialize`（序列化为中文字符串消息）

- [ ] **Step 1: 写失败测试**

在 `src-tauri/src/error.rs` 写入：

```rust
use serde::ser::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("未找到可用的 scoop 根目录")]
    NoRoot,
    #[error("根目录无效（缺少 apps/buckets/shims 目录）: {0}")]
    InvalidRoot(String),
    #[error("操作失败: {0}")]
    Io(String),
    #[error("JSON 解析错误: {0}")]
    Json(String),
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Json(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_chinese_message() {
        let v = serde_json::to_string(&AppError::NoRoot).unwrap();
        assert_eq!(v, "\"未找到可用的 scoop 根目录\"");
    }

    #[test]
    fn converts_io_error() {
        let e: AppError = std::io::Error::new(std::io::ErrorKind::NotFound, "x").into();
        assert!(e.to_string().contains("操作失败"));
    }
}
```

在 `lib.rs` 的模块声明区加：

```rust
mod error;
pub use error::AppError;
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test error 2>&1 | tail -8
```

Expected: 编译失败（`thiserror` 未安装）或 `unresolved import`——红。

- [ ] **Step 3: 添加依赖**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo add thiserror 2>&1 | tail -3
cargo add --dev tempfile 2>&1 | tail -3
```

- [ ] **Step 4: 运行测试确认通过**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
cargo test error 2>&1 | tail -8
```

Expected: `2 passed`（serializes_to_chinese_message、converts_io_error）。

- [ ] **Step 5: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/error.rs src-tauri/src/lib.rs
git commit -m "feat: 后端错误类型（中文序列化）与 thiserror/tempfile 依赖"
```

---

### Task 2: 路径与设置持久化（paths.rs / settings.rs）

**Files:**
- Create: `src-tauri/src/paths.rs`、`src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`（`mod paths; mod settings;`）
- Test: 模块内 `#[cfg(test)]`

**Interfaces:**
- Consumes: Task 1 的 `AppError`
- Produces:
  - `paths::settings_dir(appdata: &Path) -> PathBuf`（`%APPDATA%\scoopui`）
  - `paths::icon_cache_dir(localappdata: &Path) -> PathBuf`（`%LOCALAPPDATA%\scoopui\icons`，M2 用）
  - `paths::user_config_home(home: &Path) -> PathBuf`（`~/.config`）
  - `paths::default_root(home: &Path) -> PathBuf`（`~/scoop`）
  - `paths::default_global_root(program_data: &Path) -> PathBuf`（`%ProgramData%\scoop`）
  - `settings::Settings { known_roots: Vec<String>, active_root: Option<String> }`（serde，字段带 `#[serde(default)]`）
  - `settings::load(dir: &Path) -> Settings`（缺失/损坏返回默认）
  - `settings::save(dir: &Path, s: &Settings) -> Result<(), AppError>`（自动建目录，pretty JSON）

- [ ] **Step 1: 写失败测试**

`src-tauri/src/paths.rs`：

```rust
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
    }
}
```

`src-tauri/src/settings.rs`：

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::AppError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub known_roots: Vec<String>,
    #[serde(default)]
    pub active_root: Option<String>,
}

pub fn load(dir: &Path) -> Settings {
    let file = dir.join("settings.json");
    match std::fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(dir: &Path, s: &Settings) -> Result<(), AppError> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(s)?;
    std::fs::write(dir.join("settings.json"), text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_values() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Settings {
            known_roots: vec!["C:\\scoop".into()],
            active_root: Some("C:\\scoop".into()),
        };
        save(tmp.path(), &s).unwrap();
        let loaded = load(tmp.path());
        assert_eq!(loaded.known_roots, vec!["C:\\scoop".to_string()]);
        assert_eq!(loaded.active_root.as_deref(), Some("C:\\scoop"));
    }

    #[test]
    fn missing_or_corrupt_returns_default() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).known_roots.is_empty());
        std::fs::write(tmp.path().join("settings.json"), "{not json").unwrap();
        assert!(load(tmp.path()).active_root.is_none());
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test paths:: settings:: 2>&1 | tail -6
```

Expected: 编译失败（模块未声明 `mod paths; mod settings;`）——红。

- [ ] **Step 3: 在 lib.rs 声明模块**

```rust
mod paths;
mod settings;
```

- [ ] **Step 4: 运行确认通过**

Run:

```bash
cargo test paths:: settings:: 2>&1 | tail -6
```

Expected: `3 passed`。

- [ ] **Step 5: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/src/paths.rs src-tauri/src/settings.rs src-tauri/src/lib.rs
git commit -m "feat: 路径工具与设置持久化（%APPDATA%\\scoopui\\settings.json）"
```

---

### Task 3: 清单解析（domain/manifest.rs）

**Files:**
- Create: `src-tauri/src/domain/mod.rs`、`src-tauri/src/domain/manifest.rs`
- Modify: `src-tauri/src/lib.rs`（`pub mod domain;`）
- Test: 模块内 `#[cfg(test)]`

**Interfaces:**
- Consumes: `AppError`
- Produces:
  - `ManifestSummary { version: String, description/homepage/license/icon: Option<String>, depends/bin/persist: Vec<String> }`（`Serialize`）
  - `manifest::parse(text: &str) -> Result<ManifestSummary, AppError>`；`version` 缺失时报 `AppError::Other("清单缺少 version 字段")`
  - 归一化规则：`license` 支持字符串或 `{identifier}` 对象；`bin` 支持字符串、字符串数组、`[name, target]` 数组元素；`persist` 支持字符串、字符串数组、`[name, target]` 元素、对象（取 key）

- [ ] **Step 1: 写失败测试**

`src-tauri/src/domain/mod.rs`：

```rust
pub mod manifest;
```

`src-tauri/src/domain/manifest.rs`：

```rust
use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ManifestSummary {
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub icon: Option<String>,
    pub depends: Vec<String>,
    pub bin: Vec<String>,
    pub persist: Vec<String>,
}

#[derive(Deserialize)]
struct Raw {
    version: Option<String>,
    description: Option<String>,
    homepage: Option<String>,
    license: Option<serde_json::Value>,
    icon: Option<String>,
    depends: Option<serde_json::Value>,
    bin: Option<serde_json::Value>,
    persist: Option<serde_json::Value>,
}

fn strings_from(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(|item| match item {
                serde_json::Value::String(s) => vec![s.clone()],
                serde_json::Value::Array(inner) => inner
                    .first()
                    .and_then(|x| x.as_str())
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default(),
                _ => Vec::new(),
            })
            .collect(),
        serde_json::Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

fn license_from(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => map
            .get("identifier")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

pub fn parse(text: &str) -> Result<ManifestSummary, AppError> {
    let raw: Raw = serde_json::from_str(text)?;
    let version = raw
        .version
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Other("清单缺少 version 字段".into()))?;
    Ok(ManifestSummary {
        version,
        description: raw.description,
        homepage: raw.homepage,
        license: raw.license.as_ref().and_then(license_from),
        icon: raw.icon,
        depends: raw.depends.as_ref().map(strings_from).unwrap_or_default(),
        bin: raw.bin.as_ref().map(strings_from).unwrap_or_default(),
        persist: raw.persist.as_ref().map(strings_from).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_manifest() {
        let text = r#"{
            "version": "2.55.0.5",
            "description": "A free and open source distributed version control system.",
            "homepage": "https://gitforwindows.org",
            "license": { "identifier": "GPL-2.0-only", "url": "https://example.com" },
            "depends": ["less"],
            "bin": [["git", "git.exe"], "scalar.exe"],
            "persist": [["etc/gitconfig", "config"], "ssh"]
        }"#;
        let m = parse(text).unwrap();
        assert_eq!(m.version, "2.55.0.5");
        assert_eq!(m.license.as_deref(), Some("GPL-2.0-only"));
        assert_eq!(m.homepage.as_deref(), Some("https://gitforwindows.org"));
        assert_eq!(m.depends, vec!["less"]);
        assert_eq!(m.bin, vec!["git", "scalar.exe"]);
        assert_eq!(m.persist, vec!["etc/gitconfig", "ssh"]);
    }

    #[test]
    fn normalizes_string_and_object_forms() {
        let m = parse(r#"{"version":"1.0","license":"MIT","persist":{"data":"d"},"bin":"tool.exe"}"#).unwrap();
        assert_eq!(m.license.as_deref(), Some("MIT"));
        assert_eq!(m.persist, vec!["data"]);
        assert_eq!(m.bin, vec!["tool.exe"]);
    }

    #[test]
    fn rejects_manifest_without_version() {
        let err = parse(r#"{"description":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("version"));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test manifest:: 2>&1 | tail -6
```

Expected: 编译失败（`pub mod domain;` 未声明）——红。

- [ ] **Step 3: 在 lib.rs 声明模块**

```rust
pub mod domain;
```

- [ ] **Step 4: 运行确认通过**

Run:

```bash
cargo test manifest:: 2>&1 | tail -6
```

Expected: `3 passed`。

- [ ] **Step 5: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/src/domain/mod.rs src-tauri/src/domain/manifest.rs src-tauri/src/lib.rs
git commit -m "feat: 清单解析与归一化（license/bin/persist 多形态）"
```

---

### Task 4: 配置定位（domain/configfile.rs）

**Files:**
- Create: `src-tauri/src/domain/configfile.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod configfile;`）

**Interfaces:**
- Consumes: `AppError`
- Produces:
  - `ConfigFile { path: PathBuf, portable: bool }`
  - `resolve(root: &Path, user_config_home: &Path) -> ConfigFile`：`<root>\config.json` 存在 → portable；否则 `user_config_home\scoop\config.json`
  - `load(path: &Path) -> serde_json::Value`：缺失返回 `{}`；损坏返回 `{}`（容错，不报错）

- [ ] **Step 1: 写失败测试**

```rust
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub portable: bool,
}

pub fn resolve(root: &Path, user_config_home: &Path) -> ConfigFile {
    let portable = root.join("config.json");
    if portable.is_file() {
        ConfigFile { path: portable, portable: true }
    } else {
        ConfigFile {
            path: user_config_home.join("scoop").join("config.json"),
            portable: false,
        }
    }
}

pub fn load(path: &Path) -> Value {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| Value::Object(Default::default())),
        Err(_) => Value::Object(Default::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_portable_config() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("scoop");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("config.json"), "{}").unwrap();
        let c = resolve(&root, tmp.path());
        assert!(c.portable);
        assert_eq!(c.path, root.join("config.json"));
    }

    #[test]
    fn falls_back_to_user_config() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("scoop");
        std::fs::create_dir_all(&root).unwrap();
        let c = resolve(&root, tmp.path());
        assert!(!c.portable);
        assert_eq!(c.path, tmp.path().join("scoop").join("config.json"));
    }

    #[test]
    fn load_missing_or_corrupt_returns_empty_object() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(load(&tmp.path().join("nope.json")), Value::Object(Default::default()));
        std::fs::write(tmp.path().join("bad.json"), "{oops").unwrap();
        assert_eq!(load(&tmp.path().join("bad.json")), Value::Object(Default::default()));
    }
}
```

- [ ] **Step 2: 运行确认失败 → Step 3: 在 `domain/mod.rs` 加 `pub mod configfile;` → Step 4: 运行确认 `3 passed`**

Run（每步）:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test configfile:: 2>&1 | tail -6
```

- [ ] **Step 5: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/src/domain/configfile.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 配置定位（portable 优先）与容错加载"
```

---

### Task 5: 根目录探测与校验（domain/root.rs）

**Files:**
- Create: `src-tauri/src/domain/root.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod root;`）；`src-tauri/src/paths.rs` 无需改

**Interfaces:**
- Consumes: `AppError`、`domain::configfile`
- Produces:
  - `RootInfo`（`Serialize`）：`{ path: String, valid: bool, source: String, is_active: bool, app_count: usize, bucket_count: usize, has_portable_config: bool, global_dir: Option<String> }`
  - `validate(path: &Path) -> Result<(), AppError>`：要求 `<p>/apps`、`<p>/buckets`、`<p>/shims` 均为目录
  - `Candidate { path: PathBuf, source: &'static str }`
  - `candidates(env_scoop: Option<PathBuf>, config_root_path: Option<PathBuf>, known: &[PathBuf], home: &Path) -> Vec<Candidate>`：顺序 env → config → known → default；按大小写不敏感路径去重（保留先出现者）
  - `global_dir(env_global: Option<PathBuf>, config_global_path: Option<PathBuf>, program_data: &Path) -> PathBuf`：优先级同 scoop 源码
  - `info(path: &Path, source: &str, is_active: bool, global_dir: Option<&Path>) -> RootInfo`：valid 失败时 `valid=false`、其余计数为 0

- [ ] **Step 1: 写失败测试**

```rust
use std::path::{Path, PathBuf};

use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct RootInfo {
    pub path: String,
    pub valid: bool,
    pub source: String,
    pub is_active: bool,
    pub app_count: usize,
    pub bucket_count: usize,
    pub has_portable_config: bool,
    pub global_dir: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: PathBuf,
    pub source: &'static str,
}

pub fn validate(path: &Path) -> Result<(), AppError> {
    for sub in ["apps", "buckets", "shims"] {
        if !path.join(sub).is_dir() {
            return Err(AppError::InvalidRoot(path.display().to_string()));
        }
    }
    Ok(())
}

fn count_dirs(p: &Path) -> usize {
    match std::fs::read_dir(p) {
        Ok(entries) => entries.flatten().filter(|e| e.path().is_dir()).count(),
        Err(_) => 0,
    }
}

fn normalized(p: &Path) -> String {
    p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase()
}

pub fn candidates(
    env_scoop: Option<PathBuf>,
    config_root_path: Option<PathBuf>,
    known: &[PathBuf],
    home: &Path,
) -> Vec<Candidate> {
    let mut list: Vec<Candidate> = Vec::new();
    if let Some(p) = env_scoop {
        list.push(Candidate { path: p, source: "env:SCOOP" });
    }
    if let Some(p) = config_root_path {
        list.push(Candidate { path: p, source: "config:root_path" });
    }
    for p in known {
        list.push(Candidate { path: p.clone(), source: "settings" });
    }
    list.push(Candidate { path: home.join("scoop"), source: "default" });

    let mut seen: Vec<String> = Vec::new();
    list.into_iter()
        .filter(|c| {
            let key = normalized(&c.path);
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        })
        .collect()
}

pub fn global_dir(
    env_global: Option<PathBuf>,
    config_global_path: Option<PathBuf>,
    program_data: &Path,
) -> PathBuf {
    env_global
        .or(config_global_path)
        .unwrap_or_else(|| program_data.join("scoop"))
}

pub fn info(path: &Path, source: &str, is_active: bool, global_dir: Option<&Path>) -> RootInfo {
    let valid = validate(path).is_ok();
    RootInfo {
        path: path.display().to_string(),
        valid,
        source: source.to_string(),
        is_active,
        app_count: if valid { count_dirs(&path.join("apps")) } else { 0 },
        bucket_count: if valid { count_dirs(&path.join("buckets")) } else { 0 },
        has_portable_config: path.join("config.json").is_file(),
        global_dir: global_dir.map(|g| g.display().to_string()),
    }
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

    #[test]
    fn validate_requires_three_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(tmp.path());
        assert!(validate(&root).is_ok());
        std::fs::remove_dir(root.join("shims")).unwrap();
        assert!(validate(&root).is_err());
    }

    #[test]
    fn candidates_ordered_and_deduped_case_insensitive() {
        let list = candidates(
            Some(PathBuf::from("C:\\Scoop")),
            Some(PathBuf::from("D:\\scoop")),
            &[PathBuf::from("c:\\scoop"), PathBuf::from("E:\\scoop")],
            Path::new("C:\\Users\\u"),
        );
        let sources: Vec<&str> = list.iter().map(|c| c.source).collect();
        assert_eq!(sources, vec!["env:SCOOP", "config:root_path", "settings", "default"]);
        let paths: Vec<String> = list.iter().map(|c| c.path.display().to_string()).collect();
        assert_eq!(paths[2], "E:\\scoop");
    }

    #[test]
    fn global_dir_precedence() {
        let pd = Path::new("C:\\ProgramData");
        assert_eq!(
            global_dir(Some(PathBuf::from("X:\\g")), Some(PathBuf::from("Y:\\g")), pd),
            PathBuf::from("X:\\g")
        );
        assert_eq!(
            global_dir(None, Some(PathBuf::from("Y:\\g")), pd),
            PathBuf::from("Y:\\g")
        );
        assert_eq!(global_dir(None, None, pd), PathBuf::from("C:\\ProgramData\\scoop"));
    }

    #[test]
    fn info_reports_counts_and_validity() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root(tmp.path());
        std::fs::create_dir_all(root.join("apps").join("git")).unwrap();
        std::fs::create_dir_all(root.join("buckets").join("main")).unwrap();
        let i = info(&root, "test", true, None);
        assert!(i.valid);
        assert!(i.is_active);
        assert_eq!(i.app_count, 1);
        assert_eq!(i.bucket_count, 1);
        let bad = info(&tmp.path().join("nope"), "test", false, None);
        assert!(!bad.valid);
        assert_eq!(bad.app_count, 0);
    }
}
```

- [ ] **Step 2: 运行确认失败 → Step 3: 在 `domain/mod.rs` 加 `pub mod root;` → Step 4: 运行确认 `4 passed`**

Run（每步）:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test root:: 2>&1 | tail -6
```

- [ ] **Step 5: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/src/domain/root.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 根目录候选探测（env/config/known/default）与校验"
```

---

### Task 6: 已装应用扫描（domain/installed.rs）

**Files:**
- Create: `src-tauri/src/domain/installed.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod installed;`）

**Interfaces:**
- Consumes: `AppError`、`domain::manifest`
- Produces:
  - `InstalledApp`（`Serialize`）：`{ name, version, bucket: Option<String>, architecture: Option<String>, hold: bool, updated: Option<String>, description/homepage/icon: Option<String>, depends/persist/bin: Vec<String>, app_dir: String }`
  - `scan(apps_dir: &Path) -> Vec<InstalledApp>`：遍历一级子目录；仅当 `<name>/current/manifest.json` 与 `<name>/current/install.json` 同时存在才计入（自然跳过 scoop 自身）；`hold` 取 install.json 的 `hold == true`；按名称小写升序

- [ ] **Step 1: 写失败测试**

```rust
use serde::Deserialize;
use std::path::Path;

use crate::domain::manifest;

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstalledApp {
    pub name: String,
    pub version: String,
    pub bucket: Option<String>,
    pub architecture: Option<String>,
    pub hold: bool,
    pub updated: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub icon: Option<String>,
    pub depends: Vec<String>,
    pub persist: Vec<String>,
    pub bin: Vec<String>,
    pub app_dir: String,
}

#[derive(Deserialize, Default)]
struct InstallJson {
    bucket: Option<String>,
    architecture: Option<String>,
    hold: Option<bool>,
    updated: Option<String>,
}

pub fn scan(apps_dir: &Path) -> Vec<InstalledApp> {
    let mut apps: Vec<InstalledApp> = Vec::new();
    let Ok(entries) = std::fs::read_dir(apps_dir) else {
        return apps;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let current = dir.join("current");
        let manifest_path = current.join("manifest.json");
        let install_path = current.join("install.json");
        if !manifest_path.is_file() || !install_path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(m) = manifest::parse(&text) else {
            continue;
        };
        let install: InstallJson = std::fs::read_to_string(&install_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        apps.push(InstalledApp {
            name: entry.file_name().to_string_lossy().to_string(),
            version: m.version,
            bucket: install.bucket,
            architecture: install.architecture,
            hold: install.hold.unwrap_or(false),
            updated: install.updated,
            description: m.description,
            homepage: m.homepage,
            icon: m.icon,
            depends: m.depends,
            persist: m.persist,
            bin: m.bin,
            app_dir: dir.display().to_string(),
        });
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_app(root: &Path, name: &str, manifest: &str, install: Option<&str>) -> PathBuf {
        let current = root.join(name).join("current");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(current.join("manifest.json"), manifest).unwrap();
        if let Some(i) = install {
            std::fs::write(current.join("install.json"), i).unwrap();
        }
        current
    }

    #[test]
    fn scans_only_dirs_with_manifest_and_install() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = tmp.path();
        write_app(apps, "git", r#"{"version":"2.55.0.5","description":"git"}"#, Some(r#"{"bucket":"main","architecture":"64bit"}"#));
        write_app(apps, "7zip", r#"{"version":"26.00"}"#, Some(r#"{"bucket":"main"}"#));
        write_app(apps, "scoop", r#"{"version":"0.5.3"}"#, None);
        std::fs::create_dir_all(apps.join("orphan")).unwrap();

        let list = scan(apps);
        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["7zip", "git"]);
        let git = &list[1];
        assert_eq!(git.version, "2.55.0.5");
        assert_eq!(git.bucket.as_deref(), Some("main"));
        assert_eq!(git.architecture.as_deref(), Some("64bit"));
        assert!(!git.hold);
    }

    #[test]
    fn reads_hold_and_updated() {
        let tmp = tempfile::tempdir().unwrap();
        write_app(
            tmp.path(),
            "python",
            r#"{"version":"3.13.1","depends":["aria2"],"persist":"data"}"#,
            Some(r#"{"bucket":"main","hold":true,"updated":"2026-09-11T17:35:18Z"}"#),
        );
        let list = scan(tmp.path());
        assert_eq!(list.len(), 1);
        assert!(list[0].hold);
        assert_eq!(list[0].updated.as_deref(), Some("2026-09-11T17:35:18Z"));
        assert_eq!(list[0].depends, vec!["aria2"]);
        assert_eq!(list[0].persist, vec!["data"]);
    }

    #[test]
    fn missing_apps_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(scan(&tmp.path().join("nope")).is_empty());
    }
}
```

- [ ] **Step 2: 运行确认失败 → Step 3: `domain/mod.rs` 加 `pub mod installed;` → Step 4: 确认 `3 passed`**

Run（每步）:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test installed:: 2>&1 | tail -6
```

- [ ] **Step 5: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/src/domain/installed.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 已装应用扫描（manifest+install.json，含 hold）"
```

---

### Task 7: 桶扫描（domain/buckets.rs）

**Files:**
- Create: `src-tauri/src/domain/buckets.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod buckets;`）

**Interfaces:**
- Consumes: 无
- Produces:
  - `BucketInfo { name: String, source: Option<String>, is_git: bool, manifest_count: usize }`（`Serialize`）
  - `scan(buckets_dir: &Path) -> Vec<BucketInfo>`：按名称升序；`manifest_count` 递归统计 `<bucket>/bucket/**.json`
  - `git_remote(bucket_dir: &Path) -> Option<String>`：解析 `<bucket>/.git/config`（若 `.git` 为文件则读取其 `gitdir:` 指向的 config）中 `[remote "origin"]` 段的 `url` 行
  - `count_json_recursive(dir: &Path) -> usize`

- [ ] **Step 1: 写失败测试**

```rust
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct BucketInfo {
    pub name: String,
    pub source: Option<String>,
    pub is_git: bool,
    pub manifest_count: usize,
}

fn git_config_path(bucket_dir: &Path) -> Option<std::path::PathBuf> {
    let git = bucket_dir.join(".git");
    if git.is_dir() {
        Some(git.join("config"))
    } else if git.is_file() {
        let text = std::fs::read_to_string(&git).ok()?;
        let line = text.lines().find(|l| l.trim_start().starts_with("gitdir:"))?;
        let dir = line.trim_start().trim_start_matches("gitdir:").trim();
        let p = std::path::PathBuf::from(dir);
        let p = if p.is_absolute() { p } else { bucket_dir.join(p) };
        Some(p.join("config"))
    } else {
        None
    }
}

pub fn git_remote(bucket_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(git_config_path(bucket_dir)?).ok()?;
    let mut in_origin = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_origin = t.eq_ignore_ascii_case("[remote \"origin\"]");
            continue;
        }
        if in_origin {
            if let Some(rest) = t.strip_prefix("url") {
                if let Some(v) = rest.trim_start().strip_prefix('=') {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

pub fn count_json_recursive(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                count_json_recursive(&p)
            } else if p.extension().map(|x| x.eq_ignore_ascii_case("json")).unwrap_or(false) {
                1
            } else {
                0
            }
        })
        .sum()
}

pub fn scan(buckets_dir: &Path) -> Vec<BucketInfo> {
    let mut list: Vec<BucketInfo> = Vec::new();
    let Ok(entries) = std::fs::read_dir(buckets_dir) else {
        return list;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let source = git_remote(&dir);
        list.push(BucketInfo {
            name: entry.file_name().to_string_lossy().to_string(),
            is_git: source.is_some(),
            source,
            manifest_count: count_json_recursive(&dir.join("bucket")),
        });
    }
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_remote_from_config() {
        let tmp = tempfile::tempdir().unwrap();
        let bucket = tmp.path().join("main");
        std::fs::create_dir_all(bucket.join(".git")).unwrap();
        std::fs::write(
            bucket.join(".git").join("config"),
            "[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = https://gitee.com/scoop-installer/Main\n\tfetch = +refs/heads/*\n",
        )
        .unwrap();
        assert_eq!(
            git_remote(&bucket).as_deref(),
            Some("https://gitee.com/scoop-installer/Main")
        );
    }

    #[test]
    fn scans_buckets_with_counts_and_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir_all(main.join("bucket").join("sub")).unwrap();
        std::fs::create_dir_all(main.join(".git")).unwrap();
        std::fs::write(main.join(".git").join("config"), "[remote \"origin\"]\nurl = X\n").unwrap();
        std::fs::write(main.join("bucket").join("a.json"), "{}").unwrap();
        std::fs::write(main.join("bucket").join("sub").join("b.json"), "{}").unwrap();
        std::fs::write(main.join("bucket").join("readme.txt"), "x").unwrap();

        let extras = tmp.path().join("extras");
        std::fs::create_dir_all(extras.join("bucket")).unwrap();
        std::fs::write(extras.join("bucket").join("c.json"), "{}").unwrap();

        let list = scan(tmp.path());
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "extras");
        assert!(!list[0].is_git);
        assert_eq!(list[0].manifest_count, 1);
        assert_eq!(list[1].name, "main");
        assert!(list[1].is_git);
        assert_eq!(list[1].source.as_deref(), Some("X"));
        assert_eq!(list[1].manifest_count, 2);
    }

    #[test]
    fn missing_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(scan(&tmp.path().join("nope")).is_empty());
    }
}
```

- [ ] **Step 2: 运行确认失败 → Step 3: `domain/mod.rs` 加 `pub mod buckets;` → Step 4: 确认 `3 passed`**

Run（每步）:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test buckets:: 2>&1 | tail -6
```

- [ ] **Step 5: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/src/domain/buckets.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 桶扫描（git remote 解析 + 清单计数）"
```

---

### Task 8: 命令构造与流式执行（domain/scoopcmd.rs）

**Files:**
- Create: `src-tauri/src/domain/scoopcmd.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod scoopcmd;`）

**Interfaces:**
- Consumes: `AppError`
- Produces:
  - `ScoopCommand { program: String, args: Vec<String>, envs: Vec<(String, String)>, display: String }`
  - `quote_ps(s: &str) -> String`：PowerShell 单引号转义（`'` → `''`，整体包单引号）
  - `build(root: &Path, global: Option<&Path>, args: &[String]) -> Result<ScoopCommand, AppError>`：ps1 不存在时 `AppError::InvalidRoot`；envs 含 `SCOOP`，`global` 存在时含 `SCOOP_GLOBAL`
  - `StreamLine { stream: &'static str, line: String }`（`Serialize`，stream ∈ {"stdout","stderr"}）
  - `spawn(cmd: &ScoopCommand) -> Result<std::process::Child, AppError>`：Windows `CREATE_NO_WINDOW (0x08000000)`，stdout/stderr piped
  - `stream_child(child, on_line: impl FnMut(StreamLine)) -> Result<Option<i32>, AppError>`：双线程逐行读取，行按到达顺序回调；返回退出码
  - `kill_tree(pid: u32) -> bool`：`taskkill /T /F /PID`

- [ ] **Step 1: 写失败测试**

```rust
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoopCommand {
    pub program: String,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub display: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamLine {
    pub stream: &'static str,
    pub line: String,
}

pub fn quote_ps(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn build(root: &Path, global: Option<&Path>, args: &[String]) -> Result<ScoopCommand, AppError> {
    let ps1 = root.join("apps").join("scoop").join("current").join("bin").join("scoop.ps1");
    if !ps1.is_file() {
        return Err(AppError::InvalidRoot(root.display().to_string()));
    }
    let joined = args.iter().map(|a| quote_ps(a)).collect::<Vec<_>>().join(" ");
    let script = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; $OutputEncoding=[Text.Encoding]::UTF8; & {} {}",
        quote_ps(&ps1.display().to_string()),
        joined
    );
    let mut envs = vec![("SCOOP".to_string(), root.display().to_string())];
    if let Some(g) = global {
        envs.push(("SCOOP_GLOBAL".to_string(), g.display().to_string()));
    }
    let display = format!("scoop {}", args.join(" "));
    Ok(ScoopCommand {
        program: "powershell.exe".to_string(),
        args: vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-Command".to_string(),
            script,
        ],
        envs,
        display,
    })
}

pub fn spawn(cmd: &ScoopCommand) -> Result<Child, AppError> {
    let mut c = Command::new(&cmd.program);
    c.args(&cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &cmd.envs {
        c.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    Ok(c.spawn()?)
}

pub fn stream_child(
    mut child: Child,
    mut on_line: impl FnMut(StreamLine),
) -> Result<Option<i32>, AppError> {
    let out = child.stdout.take();
    let err = child.stderr.take();
    let (tx, rx) = std::sync::mpsc::channel::<StreamLine>();

    let tx_out = tx.clone();
    let h_out = out.map(|o| {
        std::thread::spawn(move || {
            for line in BufReader::new(o).lines().map_while(Result::ok) {
                let _ = tx_out.send(StreamLine { stream: "stdout", line });
            }
        })
    });
    let h_err = err.map(|e| {
        std::thread::spawn(move || {
            for line in BufReader::new(e).lines().map_while(Result::ok) {
                let _ = tx.send(StreamLine { stream: "stderr", line });
            }
        })
    });

    let drain = std::thread::spawn(move || {
        for line in rx {
            on_line(line);
        }
    });

    let status = child.wait()?;
    if let Some(h) = h_out {
        let _ = h.join();
    }
    if let Some(h) = h_err {
        let _ = h.join();
    }
    let _ = drain.join();
    Ok(status.code())
}

pub fn kill_tree(pid: u32) -> bool {
    let mut c = Command::new("taskkill.exe");
    c.args(["/T", "/F", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c.status().map(|s| s.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_root_with_ps1(base: &Path) -> std::path::PathBuf {
        let root = base.join("scoop");
        let bin = root.join("apps").join("scoop").join("current").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("scoop.ps1"), "# fake").unwrap();
        root
    }

    #[test]
    fn quotes_single_quotes_for_powershell() {
        assert_eq!(quote_ps("it's"), "'it''s'");
        assert_eq!(quote_ps("a b"), "'a b'");
    }

    #[test]
    fn build_sets_envs_and_script() {
        let tmp = tempfile::tempdir().unwrap();
        let root = make_root_with_ps1(tmp.path());
        let cmd = build(
            &root,
            Some(Path::new("C:\\ProgramData\\scoop")),
            &["install".into(), "git".into(), "--global".into()],
        )
        .unwrap();
        assert_eq!(cmd.program, "powershell.exe");
        assert!(cmd.display.contains("scoop install git --global"));
        assert!(cmd.envs.contains(&("SCOOP".into(), root.display().to_string())));
        assert!(cmd.envs.contains(&("SCOOP_GLOBAL".into(), "C:\\ProgramData\\scoop".into())));
        let script = cmd.args.last().unwrap();
        assert!(script.contains("scoop.ps1"));
        assert!(script.contains("'install' 'git' '--global'"));
    }

    #[test]
    fn build_rejects_missing_ps1() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(build(tmp.path(), None, &["list".into()]).is_err());
    }

    #[test]
    fn streams_lines_and_exit_code() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "echo hello& echo warn 1>&2& exit /b 3".into()],
            envs: vec![],
            display: "test".into(),
        };
        let child = spawn(&cmd).unwrap();
        let mut lines: Vec<StreamLine> = Vec::new();
        let code = stream_child(child, |l| lines.push(l)).unwrap();
        assert_eq!(code, Some(3));
        assert!(lines.iter().any(|l| l.stream == "stdout" && l.line.contains("hello")));
        assert!(lines.iter().any(|l| l.stream == "stderr" && l.line.contains("warn")));
    }

    #[test]
    fn kill_tree_terminates_process() {
        let cmd = ScoopCommand {
            program: "cmd.exe".into(),
            args: vec!["/C".into(), "ping -n 30 127.0.0.1 >nul".into()],
            envs: vec![],
            display: "sleep".into(),
        };
        let mut child = spawn(&cmd).unwrap();
        let pid = child.id();
        assert!(kill_tree(pid));
        let start = std::time::Instant::now();
        let _ = child.wait();
        assert!(start.elapsed().as_secs() < 10);
    }
}
```

- [ ] **Step 2: 运行确认失败 → Step 3: `domain/mod.rs` 加 `pub mod scoopcmd;` → Step 4: 确认 `5 passed`**

Run（每步）:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test scoopcmd:: 2>&1 | tail -8
```

- [ ] **Step 5: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/src/domain/scoopcmd.rs src-tauri/src/domain/mod.rs
git commit -m "feat: scoop 命令构造（目标根 ps1 + SCOOP 注入）与流式执行/杀进程树"
```

---

### Task 9: Tauri 命令层与状态（state.rs / commands/*）

**Files:**
- Create: `src-tauri/src/state.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/commands/roots.rs`、`src-tauri/src/commands/apps.rs`、`src-tauri/src/commands/buckets.rs`、`src-tauri/src/commands/config.rs`、`src-tauri/src/commands/runner.rs`
- Modify: `src-tauri/src/lib.rs`（注册状态与命令；移除 `greet` 与 `app_name` 及其测试——脚手架冒烟测试使命完成，已被更有价值的测试取代）

**Interfaces:**
- Consumes: Task 1-8 全部
- Produces（前端契约，M1b 依赖）:
  - 命令：`detect_roots() -> RootInfo[]`、`get_active_root() -> RootInfo | null`、`set_active_root(path) -> RootInfo`、`add_root(path) -> RootInfo`、`list_installed_apps() -> InstalledApp[]`、`list_buckets() -> BucketInfo[]`、`get_scoop_config() -> ScoopConfig`、`run_scoop(args: string[], taskId: string) -> void`、`cancel_task(taskId: string) -> boolean`
  - `ScoopConfig { file_path: String, portable: bool, values: serde_json::Value }`
  - 事件：`scoop-log` → `{ taskId: string, stream: "stdout"|"stderr", line: string }`；`scoop-task-end` → `{ taskId: string, code: number|null, success: boolean }`
  - 设置文件：`%APPDATA%\scoopui\settings.json`（known_roots/active_root）

- [ ] **Step 1: 实现 state.rs**

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub active_root: Mutex<Option<PathBuf>>,
    pub tasks: Mutex<HashMap<String, u32>>,
}
```

- [ ] **Step 2: 实现 commands/mod.rs（运行时上下文）**

```rust
pub mod apps;
pub mod buckets;
pub mod config;
pub mod roots;
pub mod runner;

use std::path::PathBuf;

use crate::paths;

pub struct Context {
    pub home: PathBuf,
    pub appdata: PathBuf,
    pub program_data: PathBuf,
    pub user_config_home: PathBuf,
    pub env_scoop: Option<PathBuf>,
    pub env_global: Option<PathBuf>,
    pub user_config: serde_json::Value,
}

pub fn context() -> Context {
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
    let program_data = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    let user_config_home = paths::user_config_home(&home);
    let env_scoop = std::env::var_os("SCOOP").filter(|v| !v.is_empty()).map(PathBuf::from);
    let env_global = std::env::var_os("SCOOP_GLOBAL").filter(|v| !v.is_empty()).map(PathBuf::from);
    let user_config = crate::domain::configfile::load(&user_config_home.join("scoop").join("config.json"));
    Context { home, appdata, program_data, user_config_home, env_scoop, env_global, user_config }
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
```

- [ ] **Step 3: 实现 commands/roots.rs**

```rust
use std::path::PathBuf;

use tauri::State;

use crate::commands::context;
use crate::domain::root::{self, RootInfo};
use crate::error::AppError;
use crate::settings;
use crate::state::AppState;

fn known_roots(ctx: &crate::commands::Context) -> Vec<PathBuf> {
    settings::load(&ctx.settings_dir())
        .known_roots
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn active_path(state: &AppState) -> Option<PathBuf> {
    state.active_root.lock().unwrap().clone()
}

fn build_infos(ctx: &crate::commands::Context, state: &AppState) -> Vec<RootInfo> {
    let known = known_roots(ctx);
    let active = active_path(state);
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    root::candidates(ctx.env_scoop.clone(), ctx.config_root_path(), &known, &ctx.home)
        .iter()
        .map(|c| {
            let is_active = active
                .as_ref()
                .map(|a| a.to_string_lossy().to_lowercase() == c.path.to_string_lossy().to_lowercase())
                .unwrap_or(false);
            root::info(&c.path, c.source, is_active, Some(&global))
        })
        .collect()
}

#[tauri::command]
pub fn detect_roots(state: State<'_, AppState>) -> Result<Vec<RootInfo>, AppError> {
    let ctx = context();
    Ok(build_infos(&ctx, &state))
}

#[tauri::command]
pub fn get_active_root(state: State<'_, AppState>) -> Result<Option<RootInfo>, AppError> {
    let ctx = context();
    let active = active_path(&state).or_else(|| {
        settings::load(&ctx.settings_dir()).active_root.map(PathBuf::from)
    });
    if let Some(path) = active {
        if root::validate(&path).is_ok() {
            *state.active_root.lock().unwrap() = Some(path.clone());
            let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
            return Ok(Some(root::info(&path, "settings", true, Some(&global))));
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn set_active_root(path: String, state: State<'_, AppState>) -> Result<RootInfo, AppError> {
    let ctx = context();
    let p = PathBuf::from(&path);
    root::validate(&p)?;
    *state.active_root.lock().unwrap() = Some(p.clone());
    let mut s = settings::load(&ctx.settings_dir());
    s.active_root = Some(path.clone());
    if !s.known_roots.iter().any(|k| k.to_lowercase() == path.to_lowercase()) {
        s.known_roots.push(path.clone());
    }
    settings::save(&ctx.settings_dir(), &s)?;
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    Ok(root::info(&p, "manual", true, Some(&global)))
}

#[tauri::command]
pub fn add_root(path: String, state: State<'_, AppState>) -> Result<RootInfo, AppError> {
    let ctx = context();
    let p = PathBuf::from(&path);
    root::validate(&p)?;
    let mut s = settings::load(&ctx.settings_dir());
    if !s.known_roots.iter().any(|k| k.to_lowercase() == path.to_lowercase()) {
        s.known_roots.push(path.clone());
    }
    settings::save(&ctx.settings_dir(), &s)?;
    let active = active_path(&state);
    let is_active = active
        .as_ref()
        .map(|a| a.to_string_lossy().to_lowercase() == path.to_lowercase())
        .unwrap_or(false);
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    Ok(root::info(&p, "manual", is_active, Some(&global)))
}
```

- [ ] **Step 4: 实现 commands/apps.rs / buckets.rs / config.rs**

```rust
// apps.rs
use tauri::State;

use crate::domain::installed::{self, InstalledApp};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub fn list_installed_apps(state: State<'_, AppState>) -> Result<Vec<InstalledApp>, AppError> {
    let root = state
        .active_root
        .lock()
        .unwrap()
        .clone()
        .ok_or(AppError::NoRoot)?;
    Ok(installed::scan(&root.join("apps")))
}
```

```rust
// buckets.rs
use tauri::State;

use crate::domain::buckets::{self, BucketInfo};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub fn list_buckets(state: State<'_, AppState>) -> Result<Vec<BucketInfo>, AppError> {
    let root = state
        .active_root
        .lock()
        .unwrap()
        .clone()
        .ok_or(AppError::NoRoot)?;
    Ok(buckets::scan(&root.join("buckets")))
}
```

```rust
// config.rs
use serde::Serialize;
use tauri::State;

use crate::commands::context;
use crate::domain::configfile;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct ScoopConfig {
    pub file_path: String,
    pub portable: bool,
    pub values: serde_json::Value,
}

#[tauri::command]
pub fn get_scoop_config(state: State<'_, AppState>) -> Result<ScoopConfig, AppError> {
    let root = state
        .active_root
        .lock()
        .unwrap()
        .clone()
        .ok_or(AppError::NoRoot)?;
    let ctx = context();
    let resolved = configfile::resolve(&root, &ctx.user_config_home);
    Ok(ScoopConfig {
        file_path: resolved.path.display().to_string(),
        portable: resolved.portable,
        values: configfile::load(&resolved.path),
    })
}
```

- [ ] **Step 5: 实现 commands/runner.rs**

```rust
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::commands::context;
use crate::domain::root;
use crate::domain::scoopcmd;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogPayload {
    task_id: String,
    stream: String,
    line: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EndPayload {
    task_id: String,
    code: Option<i32>,
    success: bool,
}

#[tauri::command]
pub fn run_scoop(
    app: AppHandle,
    state: State<'_, AppState>,
    args: Vec<String>,
    task_id: String,
) -> Result<(), AppError> {
    let root_path = state
        .active_root
        .lock()
        .unwrap()
        .clone()
        .ok_or(AppError::NoRoot)?;
    let ctx = context();
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    let cmd = scoopcmd::build(&root_path, Some(&global), &args)?;
    let child = scoopcmd::spawn(&cmd)?;
    state.tasks.lock().unwrap().insert(task_id.clone(), child.id());

    let app_handle = app.clone();
    let task = task_id.clone();
    std::thread::spawn(move || {
        let emit_handle = app_handle.clone();
        let emit_task = task.clone();
        let result = scoopcmd::stream_child(child, move |l| {
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

#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<bool, AppError> {
    let pid = state.tasks.lock().unwrap().get(&task_id).copied();
    match pid {
        Some(pid) => Ok(scoopcmd::kill_tree(pid)),
        None => Ok(false),
    }
}
```

注意：`stream_child` 按值接收 `Child`（Task 8 定义），runner 中直接把 `child` 移入线程闭包即可。

- [ ] **Step 6: 重写 lib.rs**

```rust
mod commands;
pub mod domain;
mod error;
mod paths;
mod settings;
mod state;

pub use error::AppError;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::roots::detect_roots,
            commands::roots::get_active_root,
            commands::roots::set_active_root,
            commands::roots::add_root,
            commands::apps::list_installed_apps,
            commands::buckets::list_buckets,
            commands::config::get_scoop_config,
            commands::runner::run_scoop,
            commands::runner::cancel_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

（删除脚手架遗留的 `greet` 命令、`app_name()` 及其测试。）

- [ ] **Step 7: 编译与全量测试**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo check 2>&1 | tail -6
cargo test 2>&1 | grep -E "^test result|running" | head -8
```

Expected: `cargo check` `Finished`；`cargo test` 全部 `test result: ok`（累计约 24 个测试）。

- [ ] **Step 8: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/src/
git commit -m "feat: Tauri 命令层（根目录/应用/桶/配置/命令执行与取消）"
```

---

### Task 10: 真实环境集成冒烟（tests/backend_smoke.rs）

**Files:**
- Create: `src-tauri/tests/backend_smoke.rs`

**Interfaces:**
- Consumes: `scoopui_lib::domain::*`（`installed`、`buckets`、`configfile`、`root`、`scoopcmd`）
- Produces: 端到端验证：真实 scoop 根上扫描应用/桶、读配置、跑 `scoop --version` 流式输出

- [ ] **Step 1: 写测试（依赖真实环境，缺失时显式跳过）**

```rust
use std::path::PathBuf;

use scoopui_lib::domain::{buckets, configfile, installed, root, scoopcmd};

fn local_root() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")?;
    let root = PathBuf::from(home).join("scoop");
    if root::validate(&root).is_ok() {
        Some(root)
    } else {
        None
    }
}

#[test]
fn scans_real_environment() {
    let Some(root_path) = local_root() else {
        eprintln!("skip: 本机无有效 scoop 根目录");
        return;
    };
    let apps = installed::scan(&root_path.join("apps"));
    println!("app_count={}", apps.len());
    assert!(!apps.is_empty(), "应至少扫出一个已装应用");
    assert!(apps.iter().all(|a| !a.version.is_empty()));
    assert!(apps.iter().all(|a| a.name != "scoop"), "scoop 自身不应计入");

    let buckets = buckets::scan(&root_path.join("buckets"));
    assert!(!buckets.is_empty());
    assert!(buckets.iter().any(|b| b.manifest_count > 0));

    let cfg = configfile::resolve(&root_path, &PathBuf::from(std::env::var_os("USERPROFILE").unwrap()).join(".config"));
    let values = configfile::load(&cfg.path);
    println!("config={} portable={}", cfg.path.display(), cfg.portable);
    assert!(values.is_object());
}

#[test]
fn runs_real_scoop_version() {
    let Some(root_path) = local_root() else {
        eprintln!("skip: 本机无有效 scoop 根目录");
        return;
    };
    let cmd = scoopcmd::build(&root_path, None, &["--version".to_string()]).unwrap();
    let child = scoopcmd::spawn(&cmd).unwrap();
    let mut lines: Vec<String> = Vec::new();
    let code = scoopcmd::stream_child(child, |l| lines.push(l.line)).unwrap();
    println!("exit={:?} lines={}", code, lines.len());
    assert_eq!(code, Some(0));
    assert!(lines.iter().any(|l| l.to_lowercase().contains("scoop")));
}
```

- [ ] **Step 2: 运行**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo test --test backend_smoke -- --nocapture 2>&1 | tail -12
```

Expected: `2 passed`；输出含 `app_count=9`（本机 9 个应用，与 scoop list 一致则更佳）与 `exit=Some(0)`。若 `app_count` 与 `scoop list` 不一致，以实际为准并在提交信息中说明（例如 scoop 自身被排除、或存在损坏目录）。

- [ ] **Step 3: 提交**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add src-tauri/tests/backend_smoke.rs
git commit -m "test: 真实环境集成冒烟（应用/桶/配置/流式命令）"
```

---

## M1a 验收清单（人工）

- [ ] `cargo test` 全绿（单元 + 集成，约 26 个测试）
- [ ] `cargo test --test backend_smoke -- --nocapture` 输出本机 9 个应用、4 个桶、`scoop --version` 退出码 0
- [ ] `cargo check` 无 error
- [ ] `pnpm test` / `pnpm build` 仍通过（未动前端）
- [ ] 前端契约稳定：命令名、事件名与 payload 形状与本计划 Task 9 Interfaces 一致

## Self-Review 记录

- 规格覆盖：设计文档 §4.1（多根）、§4.2（执行框架/流/取消）、§4.3（已装/桶/配置）、§6（错误分类基础）均有任务对应；§4.4 图标、§4.5 镜像源、§4.6/4.7/4.8 属 M2-M6。
- 占位符扫描：无 TBD；每步均含可执行命令与完整代码。
- 类型一致性：`RootInfo/InstalledApp/BucketInfo/ScoopConfig/StreamLine` 字段在 Task 5/6/7/8 定义，Task 9 命令层与 Task 10 集成测试引用一致；`stream_child` 按值接收 `Child`，Task 9 runner 已按此调用。
- 已知风险：`cargo test` 中 Windows 路径字符串断言使用 `\\` 转义；`kill_tree` 测试依赖 `taskkill` 存在（Windows 必装）；集成测试在无 scoop 的机器上显式跳过而非假通过。
