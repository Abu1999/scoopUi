# ScoopUI M4a（更新检测）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 本地更新检测：移植 scoop 的版本比较规则，比较已装版本与桶内最新版本，输出可更新列表；前端在已安装列表/详情显示「可更新」徽标（设计 §4.3/M4 验收）。

**Architecture:** `domain/versions.rs`（比较器，忠实移植 scoop `Compare-Version`/`SplitVersion`）→ `domain/updates.rs`（`compute(installed, entries)`，同名优先同桶）→ `commands/updates.rs::list_updates`（复用索引缓存；`search.rs` 抽出 `load_index` 供两处共用）→ 前端 apps store 拉取并展示徽标。不联网（可选 `scoop status` 复核留待后续，YAGNI）。

**Tech Stack:** Rust（Tauri 2）、Vue 3 + Pinia + Naive UI + Vitest。无新增依赖。

## Global Constraints

- 契约：`list_updates() -> UpdateInfo[]`；`UpdateInfo { name, current, latest, bucket, hold }`（snake_case）；reject 中文
- 版本比较语义对齐本机 scoop `lib/versions.ps1`：`+`→`-`；按 `-` 分段并把字母段独立成段；数字段数值比较；含 `.`/`_` 的段递归；预发布（alpha/beta/rc/pre）短于正式版；双 nightly 视为相等
- 更新判定：`compare_versions(当前, 最新) == Less`；同名多桶时优先已装 bucket，其次桶名字典序；`hold` 应用仍进入列表（UI 自行标注）
- 前端只经 commands.ts；UI 文案全中文；代码不加注释；不加依赖
- 验证门：`cargo test` 全绿、`pnpm test` 全绿、`pnpm build` 无错
- 初始 BASE 提交：`dff950b`

---

### Task 1: 版本比较器（domain/versions.rs）

**Files:**
- Create: `src-tauri/src/domain/versions.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod versions;`）

**Interfaces:**
- Produces: `versions::compare_versions(a: &str, b: &str) -> std::cmp::Ordering`（a 与 b 比较；`Less` 表示 b 更新）

- [ ] **Step 1: 写失败测试**

创建 `src-tauri/src/domain/versions.rs`（先只放测试，实现留 Step 3）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn equal_versions() {
        assert_eq!(compare_versions("1.8.2", "1.8.2"), Ordering::Equal);
        assert_eq!(compare_versions("2026-06", "2026-06"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0+build1", "1.0.0-build1"), Ordering::Equal);
        assert_eq!(compare_versions("nightly-20240101", "nightly-20240102"), Ordering::Equal);
    }

    #[test]
    fn numeric_segments_compare_numerically() {
        assert_eq!(compare_versions("1.8.2", "1.8.10"), Ordering::Less);
        assert_eq!(compare_versions("26.00", "26.03"), Ordering::Less);
        assert_eq!(compare_versions("2.53.0.2", "2.55.0.5"), Ordering::Less);
        assert_eq!(compare_versions("17.0.18-8", "17.0.20-101"), Ordering::Less);
        assert_eq!(compare_versions("1.8.10", "1.8.2"), Ordering::Greater);
    }

    #[test]
    fn prerelease_is_less_than_release() {
        assert_eq!(compare_versions("1.1-alpha", "1.1"), Ordering::Less);
        assert_eq!(compare_versions("1.1", "1.1-alpha"), Ordering::Greater);
        assert_eq!(compare_versions("1.0-beta2", "1.0"), Ordering::Less);
    }

    #[test]
    fn underscore_and_extra_segments() {
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0", "1.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0_1", "1.0_2"), Ordering::Less);
    }
}
```

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test versions:: --lib` → 红（函数不存在）。

- [ ] **Step 2: 注册模块**

`src-tauri/src/domain/mod.rs` 加 `pub mod versions;`（保持字母序）。

- [ ] **Step 3: 实现**

```rust
use std::cmp::Ordering;

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    compare_with(a, b, '-')
}

fn compare_with(a: &str, b: &str, delimiter: char) -> Ordering {
    let a = a.replace('+', "-");
    let b = b.replace('+', "-");
    if a == b {
        return Ordering::Equal;
    }
    let ap = split_raw(&a, delimiter);
    let bp = split_raw(&b, delimiter);
    if ap.first().map(|s| s.eq_ignore_ascii_case("nightly")).unwrap_or(false)
        && bp.first().map(|s| s.eq_ignore_ascii_case("nightly")).unwrap_or(false)
    {
        return Ordering::Equal;
    }
    for i in 0..ap.len().max(bp.len()) {
        match (ap.get(i), bp.get(i)) {
            (None, Some(v)) => {
                return if is_prerelease(v) { Ordering::Greater } else { Ordering::Less };
            }
            (Some(v), None) => {
                return if is_prerelease(v) { Ordering::Less } else { Ordering::Greater };
            }
            (Some(x), Some(y)) => {
                if x.contains('.') || y.contains('.') {
                    let r = compare_with(x, y, '.');
                    if r != Ordering::Equal {
                        return r;
                    }
                    continue;
                }
                if x.contains('_') || y.contains('_') {
                    let r = compare_with(x, y, '_');
                    if r != Ordering::Equal {
                        return r;
                    }
                    continue;
                }
                let r = compare_part(x, y);
                if r != Ordering::Equal {
                    return r;
                }
            }
            (None, None) => unreachable!(),
        }
    }
    Ordering::Equal
}

fn split_raw(version: &str, delimiter: char) -> Vec<String> {
    let mut wrapped = String::with_capacity(version.len() + 8);
    let mut in_alpha = false;
    for ch in version.chars() {
        if ch.is_ascii_alphabetic() {
            if !in_alpha {
                wrapped.push(delimiter);
                in_alpha = true;
            }
            wrapped.push(ch);
        } else {
            if in_alpha {
                wrapped.push(delimiter);
                in_alpha = false;
            }
            wrapped.push(ch);
        }
    }
    wrapped
        .split(delimiter)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_num(s: &str) -> Option<u64> {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

fn compare_part(x: &str, y: &str) -> Ordering {
    match (parse_num(x), parse_num(y)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(a), None) => a.to_string().to_ascii_lowercase().cmp(&y.to_ascii_lowercase()),
        (None, Some(b)) => x.to_ascii_lowercase().cmp(&b.to_string().to_ascii_lowercase()),
        (None, None) => x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase()),
    }
}

fn is_prerelease(part: &str) -> bool {
    let lower = part.to_ascii_lowercase();
    ["alpha", "beta", "rc", "pre"].iter().any(|k| lower.contains(k))
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test versions:: --lib 2>&1 | grep -E "test result|^test "` → 4 passed；`cargo test` 全量绿；`cargo check` 0 error。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/versions.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 版本比较器（移植 scoop Compare-Version）"
```

---

### Task 2: 更新列表计算（domain/updates.rs）

**Files:**
- Create: `src-tauri/src/domain/updates.rs`
- Modify: `src-tauri/src/domain/mod.rs`（`pub mod updates;`）

**Interfaces:**
- Consumes: `versions::compare_versions`、`installed::InstalledApp`、`index::PackageEntry`
- Produces:
  - `UpdateInfo { name: String, current: String, latest: String, bucket: String, hold: bool }`（`Debug + Clone + PartialEq + Serialize`）
  - `compute(installed: &[InstalledApp], entries: &[PackageEntry]) -> Vec<UpdateInfo>`（仅含 `current < latest`；按 name 小写升序）

- [ ] **Step 1: 写失败测试**

创建 `src-tauri/src/domain/updates.rs`（先只放测试）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::index::PackageEntry;
    use crate::domain::installed::InstalledApp;

    fn app(name: &str, version: &str, bucket: Option<&str>, hold: bool) -> InstalledApp {
        InstalledApp {
            name: name.to_string(),
            version: version.to_string(),
            bucket: bucket.map(|s| s.to_string()),
            architecture: None,
            hold,
            updated: None,
            description: None,
            homepage: None,
            icon: None,
            depends: vec![],
            persist: vec![],
            bin: vec![],
            app_dir: format!("C:\\scoop\\apps\\{name}"),
        }
    }

    fn entry(name: &str, bucket: &str, version: &str) -> PackageEntry {
        PackageEntry {
            name: name.to_string(),
            bucket: bucket.to_string(),
            version: version.to_string(),
            description: None,
            homepage: None,
            license: None,
            icon: None,
            deprecated: false,
        }
    }

    #[test]
    fn detects_newer_bucket_version() {
        let apps = vec![app("git", "2.53.0.2", Some("main"), false)];
        let entries = vec![entry("git", "main", "2.55.0.5")];
        let out = compute(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "git");
        assert_eq!(out[0].current, "2.53.0.2");
        assert_eq!(out[0].latest, "2.55.0.5");
        assert_eq!(out[0].hold, false);
    }

    #[test]
    fn no_update_when_equal_or_newer_installed() {
        let apps = vec![
            app("7zip", "26.00", Some("main"), false),
            app("python", "3.14.0", Some("main"), false),
        ];
        let entries = vec![entry("7zip", "main", "26.00"), entry("python", "main", "3.13.1")];
        assert!(compute(&apps, &entries).is_empty());
    }

    #[test]
    fn hold_app_still_listed_with_flag() {
        let apps = vec![app("git", "1.0", Some("main"), true)];
        let entries = vec![entry("git", "main", "2.0")];
        let out = compute(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert!(out[0].hold);
    }

    #[test]
    fn prefers_installed_bucket_and_matches_name_case_insensitively() {
        let apps = vec![app("Git", "1.0", Some("versions"), false)];
        let entries = vec![
            entry("git", "main", "9.0"),
            entry("git", "versions", "2.0"),
        ];
        let out = compute(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].bucket, "versions");
        assert_eq!(out[0].latest, "2.0");
    }

    #[test]
    fn missing_bucket_match_falls_back_to_first_sorted() {
        let apps = vec![app("git", "1.0", Some("gone"), false)];
        let entries = vec![entry("git", "zzz", "3.0"), entry("git", "aaa", "2.0")];
        let out = compute(&apps, &entries);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].bucket, "aaa");
    }

    #[test]
    fn sorted_by_name_and_skips_unknown() {
        let apps = vec![
            app("zeta", "1.0", Some("main"), false),
            app("alpha", "1.0", Some("main"), false),
            app("unknown", "1.0", Some("main"), false),
        ];
        let entries = vec![entry("zeta", "main", "2.0"), entry("alpha", "main", "2.0")];
        let out = compute(&apps, &entries);
        let names: Vec<&str> = out.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }
}
```

Run: `cargo test updates:: --lib` → 红。

- [ ] **Step 2: 实现**

```rust
use std::cmp::Ordering;

use serde::Serialize;

use crate::domain::index::PackageEntry;
use crate::domain::installed::InstalledApp;
use crate::domain::versions::compare_versions;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpdateInfo {
    pub name: String,
    pub current: String,
    pub latest: String,
    pub bucket: String,
    pub hold: bool,
}

pub fn compute(installed: &[InstalledApp], entries: &[PackageEntry]) -> Vec<UpdateInfo> {
    let mut out: Vec<UpdateInfo> = Vec::new();
    for app in installed {
        let mut candidates: Vec<&PackageEntry> = entries
            .iter()
            .filter(|e| e.name.eq_ignore_ascii_case(&app.name))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        candidates.sort_by(|a, b| a.bucket.to_lowercase().cmp(&b.bucket.to_lowercase()));
        let chosen = app
            .bucket
            .as_deref()
            .and_then(|b| candidates.iter().find(|e| e.bucket.eq_ignore_ascii_case(b)).copied())
            .unwrap_or(candidates[0]);
        if compare_versions(&app.version, &chosen.version) == Ordering::Less {
            out.push(UpdateInfo {
                name: app.name.clone(),
                current: app.version.clone(),
                latest: chosen.version.clone(),
                bucket: chosen.bucket.clone(),
                hold: app.hold,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}
```

- [ ] **Step 3: 运行测试确认通过**

Run: `cargo test updates:: --lib` → 6 passed；`cargo test` 全量绿；`cargo check` 0 error。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/domain/updates.rs src-tauri/src/domain/mod.rs
git commit -m "feat: 更新列表计算（本地版本对比，优先同桶）"
```

---

### Task 3: 命令 `list_updates` + 索引加载复用

**Files:**
- Modify: `src-tauri/src/commands/search.rs`（抽 `load_index`，两命令改用）
- Create: `src-tauri/src/commands/updates.rs`
- Modify: `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 2 的 `updates::{compute, UpdateInfo}`、`installed::scan`、既有 `cached_index/store_index`
- Produces:
  - `search::load_index(state, root) -> Result<Arc<Vec<PackageEntry>>, AppError>`（`pub(crate) async`）
  - 命令 `list_updates() -> Vec<UpdateInfo>`

- [ ] **Step 1: 抽 `load_index` 并保持既有测试**

`search.rs` 在 `store_index` 之后加：

```rust
pub(crate) async fn load_index(
    state: &AppState,
    root: &Path,
) -> Result<Arc<Vec<PackageEntry>>, AppError> {
    if let Some(entries) = cached_index(state, root) {
        return Ok(entries);
    }
    let build_root = root.to_path_buf();
    let built = tauri::async_runtime::spawn_blocking(move || {
        Arc::new(index::build(&build_root.join("buckets")))
    })
    .await
    .map_err(|e| AppError::Other(format!("索引构建失败: {e}")))?;
    store_index(state, root, built.clone());
    Ok(built)
}
```

`search_packages` 改为：

```rust
    let entries = load_index(&state, &root).await?;
    Ok(index::search(&entries, &query, limit.unwrap_or(50)))
```

（`refresh_package_index` 保持不变——它总是强制重建。）

- [ ] **Step 2: 实现命令**

创建 `src-tauri/src/commands/updates.rs`：

```rust
use tauri::State;

use crate::commands::roots::resolve_active_root;
use crate::commands::search::load_index;
use crate::commands::context;
use crate::domain::installed;
use crate::domain::updates::{self, UpdateInfo};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn list_updates(state: State<'_, AppState>) -> Result<Vec<UpdateInfo>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = load_index(&state, &root).await?;
    let apps = installed::scan(&root.join("apps"));
    Ok(updates::compute(&apps, &entries))
}
```

`commands/mod.rs` 加 `pub mod updates;`（字母序，`search` 之后、`runner` 之后按序放 `updates` 在末尾也可，保持列表字母序：apps, buckets, config, icons, ops, roots, runner, search, updates）。`lib.rs` 注册 `commands::updates::list_updates`。

- [ ] **Step 3: 验证与提交**

Run: `cargo test && cargo check`
Expected: 全绿（预计 83 + 4 versions + 6 updates = 93）。

```bash
git add src-tauri/src/commands/search.rs src-tauri/src/commands/updates.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: list_updates 命令与索引加载复用"
```

---

### Task 4: 前端更新徽标

**Files:**
- Modify: `src/api/types.ts`、`src/api/commands.ts`、`src/stores/apps.ts`
- Modify: `src/components/app/AppList.vue`、`src/components/app/AppDetail.vue`、`src/pages/InstalledPage.vue`
- Test: `src/__tests__/commands.spec.ts`、`src/__tests__/stores.spec.ts`、`src/__tests__/app-components.spec.ts`

**Interfaces:**
- Consumes: Task 3 的 `list_updates`
- Produces:
  - `UpdateInfo { name, current, latest, bucket, hold }` 类型
  - `listUpdates()`
  - apps store：`updates: Record<string, UpdateInfo>`、getter `updateCount`、action `updateFor(name)`

- [ ] **Step 1: 写失败测试**

`commands.spec.ts` 追加（import `listUpdates`）：

```ts
    await listUpdates();
    expect(invokeMock).toHaveBeenLastCalledWith("list_updates", undefined);
```

`stores.spec.ts`：`mocks` 增加 `listUpdates: vi.fn()`（vi.mock 返回 mocks 对象，需含新键），并追加：

```ts
  it("apps.load 拉取可更新列表", async () => {
    mocks.listInstalledApps.mockResolvedValue([appFixture]);
    mocks.listUpdates.mockResolvedValue([
      { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false },
    ]);
    const store = useAppsStore();
    await store.load();
    expect(store.updates["git"].latest).toBe("2.55.0.5");
    expect(store.updateCount).toBe(1);
    expect(store.updateFor("git")?.current).toBe("2.53.0.2");
  });

  it("apps.load 更新列表失败不影响主列表", async () => {
    mocks.listInstalledApps.mockResolvedValue([appFixture]);
    mocks.listUpdates.mockRejectedValue("未找到 scoop 根目录");
    const store = useAppsStore();
    await store.load();
    expect(store.apps).toHaveLength(1);
    expect(store.updates).toEqual({});
  });
```

`app-components.spec.ts`：AppList 用例改用带 updates 的 props：

```ts
  it("有可更新时显示徽标", () => {
    const wrapper = mount(AppList, {
      props: {
        apps: [git],
        selected: null,
        updates: { git: { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false } },
      },
    });
    expect(wrapper.text()).toContain("可更新");
  });
```

（既有 AppList 两个用例的 props 需补 `updates: {}`。）

AppDetail 用例（加到 app-components.spec 的 AppDetail describe）：

```ts
  it("可更新时展示新版本提示", () => {
    const apps = useAppsStore();
    apps.updates = { git: { name: "git", current: "2.53.0.2", latest: "2.55.0.5", bucket: "main", hold: false } };
    const wrapper = mount(AppDetail, { props: { app: git } });
    expect(wrapper.text()).toContain("可更新至 2.55.0.5");
  });
```

Run: `pnpm vitest run src/__tests__/commands.spec.ts src/__tests__/stores.spec.ts src/__tests__/app-components.spec.ts` → 红。

- [ ] **Step 2: 实现**

`types.ts` 追加：

```ts
export interface UpdateInfo {
  name: string;
  current: string;
  latest: string;
  bucket: string;
  hold: boolean;
}
```

`commands.ts` 追加：

```ts
export const listUpdates = () => invoke<UpdateInfo[]>("list_updates");
```
（import 类型补 `UpdateInfo`。）

`apps.ts`：

```ts
import { listInstalledApps, listUpdates } from "../api/commands";
import type { InstalledApp, UpdateInfo } from "../api/types";

state: () => ({
  apps: [] as InstalledApp[],
  updates: {} as Record<string, UpdateInfo>,
  ...
}),
getters: {
  selected(...) {...},
  updateCount(state): number {
    return Object.keys(state.updates).length;
  },
  updateFor(state) {
    return (name: string): UpdateInfo | null => state.updates[name] ?? null;
  },
},
load(): 在 `this.apps = await listInstalledApps();` 之后：
  try {
    const list = await listUpdates();
    this.updates = Object.fromEntries(list.map((u) => [u.name, u]));
  } catch {
    this.updates = {};
  }
```
（catch 中失败静默；`load` 既有 try/catch 仍包整体 apps 失败。）

`AppList.vue`：props 增加 `updates: Record<string, UpdateInfo>`；名称后加：

```vue
<span v-if="updates[app.name]" class="update">可更新</span>
```

样式 `.update` 参考 `.hold`（蓝色边框）：

```css
.update {
  margin-left: 6px;
  font-size: 11px;
  color: #2080f0;
  border: 1px solid #2080f0;
  border-radius: 3px;
  padding: 0 4px;
}
```

`InstalledPage.vue`：`<AppList ... :updates="apps.updates" />`。

`AppDetail.vue`：computed `const update = computed(() => (props.app ? apps.updateFor(props.app.name) : null));`；`.sub` 后加：

```vue
      <div v-if="update" class="update-line">可更新至 {{ update.latest }}</div>
```
样式 `.update-line { color: #2080f0; font-size: 13px; margin-top: 4px; }`。

- [ ] **Step 3: 运行验证**

Run: 三个聚焦 spec 全绿；`pnpm test && pnpm build` 全绿。

- [ ] **Step 4: 提交**

```bash
git add src/api/types.ts src/api/commands.ts src/stores/apps.ts src/components/app/AppList.vue src/components/app/AppDetail.vue src/pages/InstalledPage.vue src/__tests__/commands.spec.ts src/__tests__/stores.spec.ts src/__tests__/app-components.spec.ts
git commit -m "feat(ui): 已安装列表/详情展示可更新徽标"
```

---

### Task 5: 真实窗口验收（更新徽标）

**Files:** 无代码改动

**Interfaces:**
- Consumes: Task 1-4

- [ ] **Step 1: 启动 dev 并截图已安装页**

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
nohup pnpm tauri dev > "/c/Users/Administrator/AppData/Local/Temp/opencode/tauri-dev-m4a.log" 2>&1 &
for i in $(seq 1 72); do
  T=$(powershell -NoProfile -Command "(Get-Process scoopui -ErrorAction SilentlyContinue | Select-Object -First 1).MainWindowTitle" 2>/dev/null)
  if [ -n "$T" ]; then echo "WINDOW_TITLE=$T"; break; fi
  sleep 5
done
sleep 10
powershell -NoProfile -ExecutionPolicy Bypass -File "C:\Users\Administrator\AppData\Local\Temp\opencode\scoop-shot.ps1" -Out "C:\Users\Administrator\AppData\Local\Temp\opencode\scoopui-m4a-badges.png"
```

- [ ] **Step 2: 核对并关闭**

核对截图：已安装列表出现「可更新」徽标（预期 git 2.53.0.2 → 2.55.0.5、7zip 26.00 → 26.03 等）；点击带徽标应用，详情出现「可更新至 x.y.z」。数值与 `C:\Users\Administrator\scoop\buckets\*\bucket\<name>.json` 实际版本一致。

```bash
taskkill.exe //F //IM scoopui.exe 2>&1 | head -2
```

- [ ] **Step 3: 写验收报告**

`.superpowers/sdd/task-m4a-acceptance.md`：截图路径、逐应用徽标与实际版本对照、问题（如有）。

---

## Self-Review 记录

- 规格覆盖：设计 §4.3 更新检测（本地版本对比 + 可更新徽标）、M4 验收「显示可更新徽标」；联网 `scoop status` 复核与镜像/aria2 属 M4b（后续计划）。
- 占位符扫描：无 TBD；全部含代码。
- 类型一致性：`UpdateInfo` 字段在 Task 2 定义、Task 3 序列化、Task 4 镜像；`load_index` 在 Task 3 定义并被 `list_updates` 使用；`updateFor`/`updateCount` 在 Task 4 定义并使用。
- 已知取舍：`nightly` 一律视为无更新（未实现 UPDATE_NIGHTLY 配置分支，与 scoop 默认一致）；不做“已装但桶中已移除”提示。
