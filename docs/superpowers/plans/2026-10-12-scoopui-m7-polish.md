# ScoopUI M7 打磨 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 深色模式（跟随系统/浅色/深色，后端持久化）、空态补齐、任务失败的中文错误建议（errorHint）、M1-M7 手工验收清单。

**Architecture:** 后端 `Settings` 增加 `theme` 并提供 `get_app_settings`/`set_theme`；前端新增 `stores/appearance.ts`（偏好 + 系统监听 + `html.dark` 类），`App.vue` 接入 naive-ui `darkTheme`，全部硬编码浅色改为 CSS 变量（`:root` / `html.dark` 两套）；`utils/errorHint.ts` 按 stderr 关键词映射建议，`runTask` 结果附带 `hint`，各调用点失败提示拼接建议；`docs/acceptance.md` 汇总验收清单。

**Tech Stack:** Rust（Tauri 2）、Vue 3 + TypeScript + Naive UI + Pinia + Vitest。无新增依赖。

**Spec:** `docs/superpowers/specs/2026-10-12-scoopui-m5b-m7-remaining-design.md` §4

## Global Constraints

- 契约加法：仅新增字段/命令；不改既有命令参数/返回结构
- 代码不加注释；不加依赖；UI 文案全中文（术语用「软件源」）
- 主题取值仅 `system | light | dark`，默认 `system`；未知值按 system 处理（后端写入时拒绝非法值）
- 验证门：`cargo test` 全绿；`pnpm test` 全绿；`pnpm build`（vue-tsc）无错
- 初始 BASE 提交：`52e623e`
- 当前基线：Rust 144 passed + 1 ignored；前端 163 passed（24 文件）
- Windows 环境：Rust 命令前先 `export PATH="$USERPROFILE/.cargo/bin:$PATH"`

## File Structure

**后端**
- `src-tauri/src/settings.rs`（改）：`Settings.theme`
- `src-tauri/src/commands/app_settings.rs`（新）：`get_app_settings` / `set_theme`
- `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`（改）：注册

**前端**
- `src/api/types.ts`、`src/api/commands.ts`（改）：`AppSettings` 与 2 个封装
- `src/stores/appearance.ts`（新）：主题偏好与系统监听
- `src/App.vue`（改）：`darkTheme` 与 `html.dark`
- `src/pages/SettingsPage.vue`（改）：「外观」区块
- `src/style.css`（改）：CSS 变量两套 + body 背景/文字
- 硬编码颜色替换：`SideBar.vue`、`TopBar.vue`、`LogDrawer.vue`、`AppList.vue`、`PackageDetail.vue`、`StorePage.vue`、`BucketsPage.vue`、`MaintenancePage.vue`、`SettingsPage.vue`、`ProfilesPage.vue`、`BackupPage.vue`、`MigratePage.vue`（`AppIcon.vue` 不动）
- `src/utils/errorHint.ts`（新）、`src/utils/tasks.ts`（改）、`src/stores/logs.ts`（改：`TaskRecord.hint?`）
- 失败提示接入：`AppDetail.vue`、`BucketsPage.vue`、`MaintenancePage.vue`、`stores/profiles.ts`、`stores/snapshots.ts`、`stores/migrate.ts`

**测试**
- 后端：`settings.rs` 内联
- 前端：`src/__tests__/appearance.spec.ts`、`error-hint.spec.ts`（新）；`commands.spec.ts`、`app-components.spec.ts`、`tasks.spec.ts`、`buckets-page.spec.ts`（改）

---

### Task 1: 主题偏好（后端设置 + appearance store + 设置页外观区块）

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Create: `src-tauri/src/commands/app_settings.rs`
- Modify: `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`
- Modify: `src/api/types.ts`、`src/api/commands.ts`
- Create: `src/stores/appearance.ts`
- Modify: `src/App.vue`、`src/pages/SettingsPage.vue`
- Create: `src/__tests__/appearance.spec.ts`
- Modify: `src/__tests__/commands.spec.ts`、`src/__tests__/app-components.spec.ts`

**Interfaces:**
- Produces: `Settings.theme: Option<String>`；命令 `get_app_settings` / `set_theme(theme)`；`useAppearanceStore`（`theme`/`systemDark`/`effectiveTheme`/`load`/`setThemePreference`）

- [ ] **Step 1: 写失败测试**

`src-tauri/src/settings.rs` 的 `mod tests` 追加：

```rust
    #[test]
    fn roundtrip_preserves_theme() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Settings {
            known_roots: vec![],
            active_root: None,
            theme: Some("dark".to_string()),
        };
        save(tmp.path(), &s).unwrap();
        assert_eq!(load(tmp.path()).theme.as_deref(), Some("dark"));
    }
```

`src/__tests__/commands.spec.ts`：`import` 列表补 `getAppSettings, setTheme`，并追加：

```ts
  it("M7 应用设置命令名与参数符合后端契约", async () => {
    await getAppSettings();
    expect(invokeMock).toHaveBeenLastCalledWith("get_app_settings", undefined);

    await setTheme("dark");
    expect(invokeMock).toHaveBeenLastCalledWith("set_theme", { theme: "dark" });
  });
```

新建 `src/__tests__/appearance.spec.ts`：

```ts
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getAppSettings: vi.fn(),
  setTheme: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

import { useAppearanceStore } from "../stores/appearance";

describe("appearance store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("系统深色时 effectiveTheme 为 dark 并响应变化", async () => {
    let handler: ((e: { matches: boolean }) => void) | null = null;
    const mql = {
      matches: false,
      addEventListener: vi.fn((_, cb: (e: { matches: boolean }) => void) => {
        handler = cb;
      }),
    };
    vi.stubGlobal("matchMedia", vi.fn(() => mql));
    mocks.getAppSettings.mockResolvedValue({ theme: "system" });
    const store = useAppearanceStore();
    await store.load();
    expect(store.effectiveTheme).toBe("light");
    handler!({ matches: true });
    expect(store.effectiveTheme).toBe("dark");
  });

  it("浅色/深色偏好覆盖系统设置并持久化", async () => {
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: true, addEventListener: vi.fn() })));
    mocks.getAppSettings.mockResolvedValue({ theme: "light" });
    mocks.setTheme.mockResolvedValue(undefined);
    const store = useAppearanceStore();
    await store.load();
    expect(store.effectiveTheme).toBe("light");
    await store.setThemePreference("dark");
    expect(mocks.setTheme).toHaveBeenCalledWith("dark");
    expect(store.effectiveTheme).toBe("dark");
  });

  it("读取偏好失败时回退 system", async () => {
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn() })));
    mocks.getAppSettings.mockRejectedValue("读取失败");
    const store = useAppearanceStore();
    await store.load();
    expect(store.theme).toBe("system");
    expect(store.loaded).toBe(true);
  });
});
```

`src/__tests__/app-components.spec.ts`：

1) hoisted mock 区加 `getAppSettings: vi.fn().mockResolvedValue({ theme: "system" })` 与 `setTheme: vi.fn()`（并入 `otherCommands` 对象或单独 hoisted 均可，注意从 mock 工厂导出）；
2) `describe("SettingsPage")` 内追加：

```ts
  it("切换外观调用 setTheme", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const roots = useRootsStore();
    roots.active = {
      path: "C:\\scoop",
      valid: true,
      source: "env",
      is_active: true,
      app_count: 1,
      bucket_count: 1,
      has_portable_config: false,
      global_dir: null,
    };
    getScoopConfig.mockResolvedValue({ file_path: "C:\\scoop\\config.json", portable: false, values: {} });
    getMirrorInfo.mockResolvedValue(mirrorFixture);
    const wrapper = mount(SettingsPage, { global: { plugins: [pinia] } });
    await flushPromises();
    wrapper.findComponent({ name: "RadioGroup" }).vm.$emit("update:value", "dark");
    await flushPromises();
    expect(setTheme).toHaveBeenCalledWith("dark");
  });
```

（`setTheme` 从 mock 工厂导入；若 `otherCommands` 是整体展开，需在展开对象中提供 `setTheme` 并以独立常量引用。）

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/appearance.spec.ts src/__tests__/commands.spec.ts src/__tests__/app-components.spec.ts`
Expected: FAIL（`stores/appearance.ts` 与命令封装不存在；外观区块未渲染）

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib settings::`
Expected: 编译失败（`Settings` 无 `theme` 字段）

- [ ] **Step 3: 实现**

`src-tauri/src/settings.rs`：结构体加字段；两处既有测试的 `Settings` 字面量补 `theme: None`（编译器指出）：

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub known_roots: Vec<String>,
    #[serde(default)]
    pub active_root: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}
```

`src-tauri/src/commands/roots.rs` 测试内 `Settings { known_roots: vec![], active_root: Some(...) }` 字面量同步补 `theme: None`（编译器指出）。

新建 `src-tauri/src/commands/app_settings.rs`：

```rust
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
```

`src-tauri/src/commands/mod.rs` 的 `pub mod` 列表插入（字母序，`apps` 之前）：

```rust
pub mod app_settings;
```

`src-tauri/src/lib.rs` 的 `invoke_handler` 中 `commands::migrate::run_registry_uninstall` 之后追加（注意前一行为逗号）：

```rust
            commands::app_settings::get_app_settings,
            commands::app_settings::set_theme
```

`src/api/types.ts` 末尾追加：

```ts
export interface AppSettings {
  theme: string;
}
```

`src/api/commands.ts` 末尾追加（`import type` 列表补 `AppSettings`）：

```ts
export const getAppSettings = () => invoke<AppSettings>("get_app_settings");
export const setTheme = (theme: string) => invoke<void>("set_theme", { theme });
```

新建 `src/stores/appearance.ts`：

```ts
import { defineStore } from "pinia";
import { getAppSettings, setTheme } from "../api/commands";

export type ThemePreference = "system" | "light" | "dark";

function prefersDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export const useAppearanceStore = defineStore("appearance", {
  state: () => ({
    theme: "system" as ThemePreference,
    systemDark: prefersDark(),
    watching: false,
    loaded: false,
  }),
  getters: {
    effectiveTheme(state): "light" | "dark" {
      if (state.theme === "system") return state.systemDark ? "dark" : "light";
      return state.theme;
    },
  },
  actions: {
    async load() {
      try {
        const s = await getAppSettings();
        if (s.theme === "system" || s.theme === "light" || s.theme === "dark") {
          this.theme = s.theme;
        }
      } catch {
        this.theme = "system";
      }
      this.watchSystem();
      this.loaded = true;
    },
    watchSystem() {
      if (this.watching) return;
      if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
      this.watching = true;
      const media = window.matchMedia("(prefers-color-scheme: dark)");
      this.systemDark = media.matches;
      media.addEventListener("change", (e) => {
        this.systemDark = e.matches;
      });
    },
    async setThemePreference(theme: ThemePreference) {
      this.theme = theme;
      await setTheme(theme);
    },
  },
});
```

`src/App.vue` 全文替换：

```vue
<template>
  <n-config-provider
    :theme="appearance.effectiveTheme === 'dark' ? darkTheme : null"
    :theme-overrides="themeOverrides"
    :locale="zhCN"
    :date-locale="dateZhCN"
  >
    <n-message-provider>
      <AppShell />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { onMounted, watchEffect } from "vue";
import { NConfigProvider, NMessageProvider, darkTheme, dateZhCN, zhCN } from "naive-ui";
import AppShell from "./components/layout/AppShell.vue";
import { useAppearanceStore } from "./stores/appearance";

const appearance = useAppearanceStore();

const themeOverrides = {
  common: { primaryColor: "#2080f0", primaryColorHover: "#4098fc" },
};

watchEffect(() => {
  document.documentElement.classList.toggle("dark", appearance.effectiveTheme === "dark");
});

onMounted(() => {
  appearance.load();
});
</script>
```

`src/pages/SettingsPage.vue`：

1) `import` 区加：

```ts
import { NRadioButton, NRadioGroup } from "naive-ui";
import { useAppearanceStore } from "../stores/appearance";
import type { ThemePreference } from "../stores/appearance";
```

（与既有 `naive-ui` 导入行合并。）

2) `<script setup>` 中加：

```ts
const appearance = useAppearanceStore();

function onThemeChange(value: ThemePreference) {
  appearance.setThemePreference(value).catch(() => {});
}
```

3) 模板在「运行环境」区块之后插入：

```vue
    <h3>外观</h3>
    <div class="actions">
      <n-radio-group :value="appearance.theme" size="small" @update:value="onThemeChange">
        <n-radio-button value="system">跟随系统</n-radio-button>
        <n-radio-button value="light">浅色</n-radio-button>
        <n-radio-button value="dark">深色</n-radio-button>
      </n-radio-group>
    </div>
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 145 passed + 1 ignored（144 + 1）

Run: `pnpm vitest run src/__tests__/appearance.spec.ts src/__tests__/commands.spec.ts src/__tests__/app-components.spec.ts`
Expected: PASS；再跑 `pnpm test` 应 163 + appearance 3 + commands 1 = 167 passed

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/settings.rs src-tauri/src/commands/app_settings.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/src/commands/roots.rs src/api/types.ts src/api/commands.ts src/stores/appearance.ts src/App.vue src/pages/SettingsPage.vue src/__tests__/appearance.spec.ts src/__tests__/commands.spec.ts src/__tests__/app-components.spec.ts
git commit -m "feat: 深色模式偏好与外观设置"
```

---

### Task 2: 深色样式变量适配与空态补齐

**Files:**
- Modify: `src/style.css`
- Modify（仅颜色字面量替换）：`src/components/layout/SideBar.vue`、`TopBar.vue`、`LogDrawer.vue`、`src/components/app/AppList.vue`、`src/components/store/PackageDetail.vue`、`src/pages/StorePage.vue`、`BucketsPage.vue`、`MaintenancePage.vue`、`SettingsPage.vue`、`ProfilesPage.vue`、`BackupPage.vue`、`MigratePage.vue`
- Modify: `src/__tests__/buckets-page.spec.ts`

**Interfaces:**
- Consumes: Task 1 的 `html.dark` 类（由 `App.vue` 切换）
- Produces: CSS 变量两套；`BucketsPage` 空态

- [ ] **Step 1: 写失败测试**

`src/__tests__/buckets-page.spec.ts` 追加：

```ts
  it("无软件源时显示空态", async () => {
    mocks.listBuckets.mockResolvedValue([]);
    const wrapper = mount(BucketsPage);
    await flushPromises();
    expect(wrapper.text()).toContain("还没有软件源");
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/buckets-page.spec.ts`
Expected: FAIL（空态文案不存在）

- [ ] **Step 3: 实现**

`src/style.css` 末尾追加：

```css
:root {
  --bg: #ffffff;
  --bg-soft: #f7f7f9;
  --bg-hover: #f5f7fa;
  --bg-active: #eaf3ff;
  --border: #efeff5;
  --text: #333333;
  --text-dim: #888888;
}

html.dark {
  --bg: #101014;
  --bg-soft: #18181c;
  --bg-hover: #1f1f26;
  --bg-active: #1c2c44;
  --border: #2a2a30;
  --text: #e0e0e6;
  --text-dim: #9a9aa5;
}

body {
  background: var(--bg);
  color: var(--text);
}
```

对以下 12 个文件做纯字面量替换（`AppIcon.vue` 不动）：

| 旧字面量 | 新值 |
|---|---|
| `#efeff5` | `var(--border)` |
| `#f5f5f7` | `var(--border)` |
| `#f5f7fa` | `var(--bg-hover)` |
| `#eaf3ff` | `var(--bg-active)` |
| `#f7f7f9` | `var(--bg-soft)` |
| `#333` / `#333333` / `#444` / `#555` | `var(--text)` |
| `#666` / `#888` / `#999` | `var(--text-dim)` |
| `white` 或 `#fff` 且用于背景 | `var(--bg)` |

文件清单：`src/components/layout/SideBar.vue`、`src/components/layout/TopBar.vue`、`src/components/layout/LogDrawer.vue`、`src/components/app/AppList.vue`、`src/components/store/PackageDetail.vue`、`src/pages/StorePage.vue`、`src/pages/BucketsPage.vue`、`src/pages/MaintenancePage.vue`、`src/pages/SettingsPage.vue`、`src/pages/ProfilesPage.vue`、`src/pages/BackupPage.vue`、`src/pages/MigratePage.vue`。

注意：`#f0a020`（锁定徽标）、`#2080f0`（主色/更新徽标）、`#18a058`（全局徽标）、`#d03050`（错误）等语义色保持不动；`AppIcon.vue` 中的 `#fff` 是头像文字色，保持不动。

`src/pages/BucketsPage.vue`：模板在 `<n-table` 之前插入空态：

```vue
    <div v-if="store.buckets.length === 0" class="empty">还没有软件源，添加一个开始使用</div>
```

样式追加：

```css
.empty {
  padding: 24px;
  color: var(--text-dim);
  text-align: center;
}
```

（若该文件已有 `.empty` 样式，复用不重复添加。）

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm test`
Expected: 167 + 1 = 168 passed

- [ ] **Step 5: 提交**

```bash
git add src/style.css src/components/layout/SideBar.vue src/components/layout/TopBar.vue src/components/layout/LogDrawer.vue src/components/app/AppList.vue src/components/store/PackageDetail.vue src/pages/StorePage.vue src/pages/BucketsPage.vue src/pages/MaintenancePage.vue src/pages/SettingsPage.vue src/pages/ProfilesPage.vue src/pages/BackupPage.vue src/pages/MigratePage.vue src/__tests__/buckets-page.spec.ts
git commit -m "feat(ui): 深色样式变量与空态补齐"
```

---

### Task 3: 错误提示分类（errorHint + runTask 接入）

**Files:**
- Create: `src/utils/errorHint.ts`
- Modify: `src/stores/logs.ts`（`TaskRecord.hint?`）
- Modify: `src/utils/tasks.ts`
- Modify: `src/components/app/AppDetail.vue`、`src/pages/BucketsPage.vue`、`src/pages/MaintenancePage.vue`
- Modify: `src/stores/profiles.ts`、`src/stores/snapshots.ts`、`src/stores/migrate.ts`
- Create: `src/__tests__/error-hint.spec.ts`
- Modify: `src/__tests__/tasks.spec.ts`

**Interfaces:**
- Consumes: logs store 的 `lines`（含 stderr 行）
- Produces: `errorHint(lines: string[]) -> string | null`；`TaskRecord.hint?: string | null`；失败提示统一附加建议

- [ ] **Step 1: 写失败测试**

新建 `src/__tests__/error-hint.spec.ts`：

```ts
import { describe, expect, it } from "vitest";
import { errorHint } from "../utils/errorHint";

describe("errorHint", () => {
  it("网络类 stderr 给出网络建议", () => {
    expect(errorHint(["Could not resolve host: github.com"])).toContain("网络");
  });
  it("哈希校验失败给出镜像建议", () => {
    expect(errorHint(["Hash check failed for git"])).toContain("镜像");
  });
  it("权限不足给出管理员建议", () => {
    expect(errorHint(["Access is denied"])).toContain("管理员");
  });
  it("manifest 冲突给出卸载建议", () => {
    expect(errorHint(["git is already installed. Use 'scoop update git'"])).toContain("卸载");
  });
  it("无匹配与空输入返回 null", () => {
    expect(errorHint(["some random output"])).toBeNull();
    expect(errorHint([])).toBeNull();
  });
});
```

`src/__tests__/tasks.spec.ts` 追加：

```ts
  it("失败任务附带错误建议", async () => {
    const logs = useLogsStore();
    let id = "";
    const pending = runTask("安装 git", async (taskId) => {
      id = taskId;
    });
    await flushPromises();
    logs.append({ taskId: id, stream: "stderr", line: "Hash check failed for git" });
    logs.end({ taskId: id, code: 1, success: false });
    const record = await pending;
    expect(record.hint).toContain("镜像");
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/error-hint.spec.ts src/__tests__/tasks.spec.ts`
Expected: FAIL（`utils/errorHint.ts` 不存在；`record.hint` 为 undefined）

- [ ] **Step 3: 实现**

新建 `src/utils/errorHint.ts`：

```ts
const RULES: Array<{ keys: string[]; hint: string }> = [
  {
    keys: [
      "timed out",
      "timeout",
      "could not resolve",
      "connection",
      "econnreset",
      "无法连接",
      "临时故障",
      "下载失败",
    ],
    hint: "检查网络或切换下载加速代理后重试",
  },
  {
    keys: ["hash check failed", "hash mismatch", "哈希校验失败", "哈希不匹配"],
    hint: "镜像未同步：运行 scoop update 或清理缓存后重试",
  },
  {
    keys: ["access is denied", "拒绝访问", "requires admin", "admin rights", "需要管理员"],
    hint: "权限不足：以管理员身份运行或改用全局安装",
  },
  {
    keys: ["already installed", "already exists", "conflict", "冲突"],
    hint: "先卸载同名应用再试",
  },
];

export function errorHint(lines: string[]): string | null {
  if (lines.length === 0) return null;
  const text = lines.join("\n").toLowerCase();
  for (const rule of RULES) {
    if (rule.keys.some((k) => text.includes(k))) return rule.hint;
  }
  return null;
}
```

`src/stores/logs.ts`：`TaskRecord` 增加字段：

```ts
export interface TaskRecord {
  taskId: string;
  label: string;
  success: boolean;
  code: number | null;
  at: number;
  hint?: string | null;
}
```

`src/utils/tasks.ts`：导入 `errorHint`，`done` 的 resolve 改为：

```ts
        if (running === false) {
          if (timer) clearTimeout(timer);
          stop();
          const record =
            logs.history.find((r) => r.taskId === taskId) ?? {
              taskId,
              label,
              success: false,
              code: null,
              at: Date.now(),
            };
          const hint = record.success
            ? null
            : errorHint(
                logs.lines
                  .filter((l) => l.taskId === taskId && l.stream === "stderr")
                  .map((l) => l.line),
              );
          resolve({ ...record, hint });
        }
```

调用点（失败提示附加建议；无建议时保持原文案，既有测试不受影响）：

`src/components/app/AppDetail.vue`、`src/pages/BucketsPage.vue`、`src/pages/MaintenancePage.vue` 三处：

```ts
    message.error(
      record.hint
        ? `操作失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
        : `操作失败（退出码 ${record.code ?? "未知"}），详见任务日志`,
    );
```

`src/stores/profiles.ts`（applyError 赋值处）、`src/stores/snapshots.ts`（3 处 restoreError 赋值）：

```ts
          this.applyError = record.hint
            ? `步骤失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
            : `步骤失败（退出码 ${record.code ?? "未知"}）`;
```

（`snapshots.ts` 中变量名为 `this.restoreError`；其余相同。）

`src/stores/migrate.ts` 失败返回：

```ts
      return {
        ok: false,
        warning,
        error: record.hint
          ? `步骤失败（退出码 ${record.code ?? "未知"}）：${record.hint}`
          : `步骤失败（退出码 ${record.code ?? "未知"}）`,
      };
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/error-hint.spec.ts src/__tests__/tasks.spec.ts`
Expected: PASS（error-hint 5 + tasks 5+1）

Run: `pnpm test`
Expected: 168 + 6 = 174 passed

- [ ] **Step 5: 提交**

```bash
git add src/utils/errorHint.ts src/stores/logs.ts src/utils/tasks.ts src/components/app/AppDetail.vue src/pages/BucketsPage.vue src/pages/MaintenancePage.vue src/stores/profiles.ts src/stores/snapshots.ts src/stores/migrate.ts src/__tests__/error-hint.spec.ts src/__tests__/tasks.spec.ts
git commit -m "feat(ui): 任务失败错误建议"
```

---

### Task 4: 验收清单与全量验证

**Files:**
- Create: `docs/acceptance.md`

- [ ] **Step 1: 编写验收清单**

新建 `docs/acceptance.md`：

```markdown
# ScoopUI 手工验收清单（M1-M7）

## 一、基础与已安装页
1. 启动 `pnpm tauri dev` 出主窗口；顶栏可切换根目录，切换后列表刷新
2. 已安装页：列表/详情正常；锁定徽标、可更新徽标显示正确
3. 全局应用（SCOOP_GLOBAL）带「全局」徽标；更新/卸载/锁定对全局应用生效
4. 卸载要求确认；失败时提示错误（有日志抽屉输出）

## 二、商店与搜索
5. 输入关键词秒出结果（离线索引）；列表/卡片视图切换正常
6. 无结果与未搜索状态有空态文案；刷新索引后包数量显示
7. 安装：可选版本/全局；进度与结果见日志抽屉，可取消

## 三、软件源与维护
8. 软件源管理：添加/删除（确认）/更新 Scoop 与软件源；空态文案显示
9. 维护页：清缓存、清旧版本、checkup、reset 均可用且确认弹窗正确

## 四、镜像与加速（设置页）
10. 软件源镜像切换（scoop 自身/各软件源）生效
11. 下载加速 URL_PROXY 设置与连通测试；出口未知时显示中性提示
12. proxy 与 aria2 设置可写入并生效（aria2 可一键安装）

## 五、配置集
13. 另存当前状态 → 列表/详情正确；重复名称报错
14. 差异区显示缺少/多余软件源与应用、设置变化
15. 应用：settings→软件源→应用逐条执行，失败即停、可重试；完成后差异清空

## 六、备份与恢复
16. 创建快照 → 列表显示时间/根路径/计数
17. 恢复配置与软件源：config.json 被写回且旧文件备份为 config.json.bak；软件源 remote 重设
18. 缺失应用可单条安装或全部重装（失败即停、剩余待执行）
19. 无 config 快照仍可恢复软件源；root 不一致时警示但不阻断
20. 删除快照需确认且列表刷新

## 七、环境迁移
21. 扫描本机软件 → 高/中/低/未匹配分组；统计数字正确
22. 已由 scoop 管理（含全局目录）的软件默认隐藏；「显示已管理条目」可见且带标记
23. 「查看」跳转商店并自动搜索
24. 「安装」成功；勾选「安装前先卸载旧版」时先执行 UninstallString（失败仅警告继续）

## 八、打磨
25. 外观：跟随系统/浅色/深色三态切换即时生效；重启后保持
26. 深色模式下各页面背景/边框/文字可读，无浅色残留块
27. 断网等失败场景：错误提示带中文建议（网络/镜像/权限/冲突）
```

- [ ] **Step 2: 后端全量测试**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 145 passed + 1 ignored、0 failed

- [ ] **Step 3: 前端全量测试与构建**

Run: `pnpm test`
Expected: 174 passed

Run: `pnpm build`
Expected: `vue-tsc` 无错误、vite build 成功

- [ ] **Step 4: 提交与修复收尾**

```bash
git add docs/acceptance.md
git commit -m "docs: M1-M7 手工验收清单"
```

若验证发现问题：修复 → 回到 Step 2 重新验证 → 追加提交（`fix: ...`）。

---

## Self-Review 记录

- 规格覆盖：§4.1 深色模式（Task 1 偏好与接入 + Task 2 变量适配）、§4.2 空态（Task 2）、§4.3 错误提示分类（Task 3）、§4.4 验收清单（Task 4）。
- 占位符扫描：无 TBD；替换映射为穷举表；测试与实现均给出完整代码。
- 类型一致性：`ThemePreference` 在 store 导出、SettingsPage 引用一致；`AppSettings.theme` 前后端字段一致；`TaskRecord.hint` 可选字段不破坏既有构造点；`errorHint` 在 tasks.ts 与 spec 中同名同签名。
- 已知取舍：`html.dark` 类由 `App.vue` 的 watchEffect 管理（测试环境 jsdom 支持 classList）；系统监听仅注册一次（`watching` 守卫），不支持清理（应用生命周期内常驻）；颜色替换不动语义色与头像色。

