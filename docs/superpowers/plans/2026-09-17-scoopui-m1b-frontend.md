# ScoopUI M1b（前端三栏界面）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现三栏主界面（侧边栏 + 内容区 + 详情面板 + 底部日志抽屉）：根目录切换、已装应用列表与详情、桶列表、配置查看、实时日志流，全部消费 M1a 已冻结的 Tauri 契约。

**Architecture:** Vue 3 + TS + Pinia + Naive UI。`src/api/`（types + 类型化命令包装，唯一后端出口）→ `src/stores/`（roots/apps/buckets/logs/ui）→ `src/components/`（layout + app 组件）→ `src/pages/`（页面）。事件监听统一在 logs store 初始化时注册。无 vue-router：用 ui store 的 `activePage` 切换页面。

**Tech Stack:** Vue 3.5 + TypeScript + Vite + Naive UI + Pinia + Vitest（jsdom）。

## Global Constraints

- **契约对齐（最重要）**：事件 payload 为 camelCase（`taskId`/`stream`/`line`、`taskId`/`code`/`success`）；`RootInfo`/`InstalledApp`/`BucketInfo`/`ScoopConfig` 字段为 **snake_case**（`is_active`/`app_count`/`bucket_count`/`has_portable_config`/`global_dir`/`file_path`），TS 接口不得改名
- `get_active_root` 返回 `RootInfo | null`；命令 reject 为中文 string（`String(e)` 即用户可读信息）
- 事件监听必须在 `run_scoop` invoke **之前**注册（logs store `init()` 在 AppShell onMounted 里最先调用）
- UI 文案全中文；代码不加注释
- 前端只能通过 `src/api/commands.ts` 调用后端；不得直接 import `@tauri-apps/api/core`（`src/api/tauri.ts` 除外）
- 不加除清单外的依赖；已有：naive-ui、pinia、@tauri-apps/api、@tauri-apps/plugin-dialog、@tauri-apps/plugin-opener、vitest、@vue/test-utils、jsdom
- 验证门：`pnpm test` 全绿、`pnpm build`（vue-tsc）无错、`pnpm tauri dev` 窗口标题 `ScoopUI` 且界面可见
- 初始 BASE 提交：2993fb5

---

### Task 1: vitest jsdom + 类型镜像 + 首字母头像工具

**Files:**
- Modify: `vite.config.ts`（vitest 配置）
- Create: `src/api/types.ts`、`src/utils/avatar.ts`、`src/__tests__/avatar.spec.ts`
- Test: `src/__tests__/avatar.spec.ts`

**Interfaces:**
- Consumes: 无
- Produces:
  - `RootInfo`、`InstalledApp`、`BucketInfo`、`ScoopConfig`、`ScoopLogEvent`、`ScoopTaskEndEvent`（`src/api/types.ts`，后续所有任务引用）
  - `avatarColor(name: string): string`、`avatarLetter(name: string): string`（`src/utils/avatar.ts`）

- [ ] **Step 1: 配置 vitest（jsdom）**

`vite.config.ts` 改为从 `vitest/config` 导入并加 test 块（保留原有 tauri dev 配置）：

```ts
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  test: {
    environment: "jsdom",
  },
});
```

- [ ] **Step 2: 写失败测试（avatar）**

`src/__tests__/avatar.spec.ts`：

```ts
import { describe, expect, it } from "vitest";
import { avatarColor, avatarLetter } from "../utils/avatar";

describe("avatarLetter", () => {
  it("取首字符并大写", () => {
    expect(avatarLetter("git")).toBe("G");
    expect(avatarLetter("7zip")).toBe("7");
  });

  it("空字符串回退问号", () => {
    expect(avatarLetter("   ")).toBe("?");
  });
});

describe("avatarColor", () => {
  it("同名稳定且来自固定色板", () => {
    const c1 = avatarColor("python");
    const c2 = avatarColor("python");
    expect(c1).toBe(c2);
    expect(c1).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it("不同名可给出不同颜色", () => {
    const names = ["git", "7zip", "python", "cmake", "mingw", "flutter", "dark", "android-clt"];
    expect(new Set(names.map(avatarColor)).size).toBeGreaterThan(1);
  });
});
```

Run: `pnpm vitest run src/__tests__/avatar.spec.ts` → 失败（模块不存在）。

- [ ] **Step 3: 创建 types.ts 与 avatar.ts**

`src/api/types.ts`：

```ts
export interface RootInfo {
  path: string;
  valid: boolean;
  source: string;
  is_active: boolean;
  app_count: number;
  bucket_count: number;
  has_portable_config: boolean;
  global_dir: string | null;
}

export interface InstalledApp {
  name: string;
  version: string;
  bucket: string | null;
  architecture: string | null;
  hold: boolean;
  updated: string | null;
  description: string | null;
  homepage: string | null;
  icon: string | null;
  depends: string[];
  persist: string[];
  bin: string[];
  app_dir: string;
}

export interface BucketInfo {
  name: string;
  source: string | null;
  is_git: boolean;
  manifest_count: number;
}

export interface ScoopConfig {
  file_path: string;
  portable: boolean;
  values: unknown;
}

export interface ScoopLogEvent {
  taskId: string;
  stream: "stdout" | "stderr";
  line: string;
}

export interface ScoopTaskEndEvent {
  taskId: string;
  code: number | null;
  success: boolean;
}
```

`src/utils/avatar.ts`：

```ts
const PALETTE = [
  "#2080f0",
  "#18a058",
  "#f0a020",
  "#d03050",
  "#8a2be2",
  "#00b4d8",
  "#e8590c",
  "#2b8a3e",
];

export function avatarColor(name: string): string {
  let h = 0;
  for (const ch of name) {
    h = (h * 31 + (ch.codePointAt(0) ?? 0)) >>> 0;
  }
  return PALETTE[h % PALETTE.length];
}

export function avatarLetter(name: string): string {
  const ch = [...name.trim()][0] ?? "?";
  return ch.toUpperCase();
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/avatar.spec.ts` → `6 passed`（4 个用例：2+2）。

同时 `pnpm build` 通过。

- [ ] **Step 5: 提交**

```bash
git add vite.config.ts src/api/types.ts src/utils/avatar.ts src/__tests__/avatar.spec.ts
git commit -m "feat(ui): vitest jsdom 配置、后端类型镜像与首字母头像工具"
```

---

### Task 2: 类型化命令包装 + ui/logs store

**Files:**
- Create: `src/api/commands.ts`、`src/stores/ui.ts`、`src/stores/logs.ts`、`src/__tests__/commands.spec.ts`、`src/__tests__/logs.spec.ts`

**Interfaces:**
- Consumes: Task 1 的 types；`src/api/tauri.ts` 的 `invoke`
- Produces:
  - `detectRoots()`、`getActiveRoot()`、`setActiveRoot(path)`、`addRoot(path)`、`listInstalledApps()`、`listBuckets()`、`getScoopConfig()`、`runScoop(args, taskId)`、`cancelTask(taskId)`（`src/api/commands.ts`，后续所有 store/组件引用）
  - `useUiStore`（state：`activePage: PageId`、`logDrawerOpen: boolean`；actions：`setPage`、`toggleLogs`）
  - `useLogsStore`（state：`lines: LogLine[]`、`running: Record<string, boolean>`、`history: TaskRecord[]`；actions：`init()`、`start(taskId, label)`、`append(e)`、`end(e)`、`clear()`）
  - `PageId = "installed" | "store" | "buckets" | "maintenance" | "migrate" | "profiles" | "backup" | "settings"`

- [ ] **Step 1: 写失败测试（commands）**

`src/__tests__/commands.spec.ts`：

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  addRoot,
  cancelTask,
  detectRoots,
  getActiveRoot,
  getScoopConfig,
  listBuckets,
  listInstalledApps,
  runScoop,
  setActiveRoot,
} from "../api/commands";

describe("commands 包装", () => {
  beforeEach(() => invokeMock.mockReset().mockResolvedValue(null));

  it("命令名与参数符合后端契约", async () => {
    await detectRoots();
    expect(invokeMock).toHaveBeenLastCalledWith("detect_roots", undefined);

    await getActiveRoot();
    expect(invokeMock).toHaveBeenLastCalledWith("get_active_root", undefined);

    await setActiveRoot("C:\\scoop");
    expect(invokeMock).toHaveBeenLastCalledWith("set_active_root", { path: "C:\\scoop" });

    await addRoot("D:\\scoop");
    expect(invokeMock).toHaveBeenLastCalledWith("add_root", { path: "D:\\scoop" });

    await listInstalledApps();
    expect(invokeMock).toHaveBeenLastCalledWith("list_installed_apps", undefined);

    await listBuckets();
    expect(invokeMock).toHaveBeenLastCalledWith("list_buckets", undefined);

    await getScoopConfig();
    expect(invokeMock).toHaveBeenLastCalledWith("get_scoop_config", undefined);

    await runScoop(["install", "git"], "t-1");
    expect(invokeMock).toHaveBeenLastCalledWith("run_scoop", { args: ["install", "git"], taskId: "t-1" });

    await cancelTask("t-1");
    expect(invokeMock).toHaveBeenLastCalledWith("cancel_task", { taskId: "t-1" });
  });
});
```

- [ ] **Step 2: 写失败测试（logs store）**

`src/__tests__/logs.spec.ts`：

```ts
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

type Handler = (e: { payload: unknown }) => void;
const listeners: Record<string, Handler> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    listeners[name] = handler;
    return Promise.resolve(() => {});
  }),
}));

import { useLogsStore } from "../stores/logs";

describe("logs store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    for (const k of Object.keys(listeners)) delete listeners[k];
  });

  it("init 注册两个事件监听", async () => {
    const store = useLogsStore();
    await store.init();
    expect(Object.keys(listeners).sort()).toEqual(["scoop-log", "scoop-task-end"]);
  });

  it("收到 scoop-log 追加行", async () => {
    const store = useLogsStore();
    await store.init();
    listeners["scoop-log"]({ payload: { taskId: "t1", stream: "stdout", line: "hello" } });
    expect(store.lines.at(-1)).toMatchObject({ taskId: "t1", stream: "stdout", line: "hello" });
  });

  it("start/end 维护 running 与 history", async () => {
    const store = useLogsStore();
    await store.init();
    store.start("t1", "scoop --version");
    expect(store.running["t1"]).toBe(true);
    listeners["scoop-task-end"]({ payload: { taskId: "t1", code: 0, success: true } });
    expect(store.running["t1"]).toBe(false);
    expect(store.history[0]).toMatchObject({ taskId: "t1", label: "scoop --version", success: true });
  });
});
```

Run: `pnpm vitest run src/__tests__/commands.spec.ts src/__tests__/logs.spec.ts` → 失败（模块不存在）。

- [ ] **Step 3: 实现 commands.ts**

```ts
import type {
  BucketInfo,
  InstalledApp,
  RootInfo,
  ScoopConfig,
} from "./types";
import { invoke } from "./tauri";

export const detectRoots = () => invoke<RootInfo[]>("detect_roots");
export const getActiveRoot = () => invoke<RootInfo | null>("get_active_root");
export const setActiveRoot = (path: string) => invoke<RootInfo>("set_active_root", { path });
export const addRoot = (path: string) => invoke<RootInfo>("add_root", { path });
export const listInstalledApps = () => invoke<InstalledApp[]>("list_installed_apps");
export const listBuckets = () => invoke<BucketInfo[]>("list_buckets");
export const getScoopConfig = () => invoke<ScoopConfig>("get_scoop_config");
export const runScoop = (args: string[], taskId: string) =>
  invoke<void>("run_scoop", { args, taskId });
export const cancelTask = (taskId: string) => invoke<boolean>("cancel_task", { taskId });
```

- [ ] **Step 4: 实现 ui.ts 与 logs.ts**

`src/stores/ui.ts`：

```ts
import { defineStore } from "pinia";

export type PageId =
  | "installed"
  | "store"
  | "buckets"
  | "maintenance"
  | "migrate"
  | "profiles"
  | "backup"
  | "settings";

export const useUiStore = defineStore("ui", {
  state: () => ({
    activePage: "installed" as PageId,
    logDrawerOpen: false,
  }),
  actions: {
    setPage(page: PageId) {
      this.activePage = page;
    },
    toggleLogs() {
      this.logDrawerOpen = !this.logDrawerOpen;
    },
  },
});
```

`src/stores/logs.ts`：

```ts
import { listen } from "@tauri-apps/api/event";
import { defineStore } from "pinia";
import type { ScoopLogEvent, ScoopTaskEndEvent } from "../api/types";

export interface LogLine {
  taskId: string;
  stream: "stdout" | "stderr" | "meta";
  line: string;
}

export interface TaskRecord {
  taskId: string;
  label: string;
  success: boolean;
  code: number | null;
  at: number;
}

let initialized = false;

export const useLogsStore = defineStore("logs", {
  state: () => ({
    lines: [] as LogLine[],
    running: {} as Record<string, boolean>,
    labels: {} as Record<string, string>,
    history: [] as TaskRecord[],
  }),
  actions: {
    async init() {
      if (initialized) return;
      initialized = true;
      await listen<ScoopLogEvent>("scoop-log", (event) => this.append(event.payload));
      await listen<ScoopTaskEndEvent>("scoop-task-end", (event) => this.end(event.payload));
    },
    start(taskId: string, label: string) {
      this.labels[taskId] = label;
      this.running[taskId] = true;
      this.lines.push({ taskId, stream: "meta", line: `▶ ${label}` });
    },
    append(e: ScoopLogEvent) {
      this.lines.push({ taskId: e.taskId, stream: e.stream, line: e.line });
    },
    end(e: ScoopTaskEndEvent) {
      this.running[e.taskId] = false;
      const label = this.labels[e.taskId] ?? e.taskId;
      this.history.unshift({
        taskId: e.taskId,
        label,
        success: e.success,
        code: e.code,
        at: Date.now(),
      });
      this.lines.push({
        taskId: e.taskId,
        stream: "meta",
        line: e.success ? `■ ${label} 完成` : `■ ${label} 失败（退出码 ${e.code ?? "未知"}）`,
      });
    },
    clear() {
      this.lines = [];
    },
  },
});
```

注：`▶` / `■` 为几何符号，非 emoji。

- [ ] **Step 5: 运行测试确认通过**

Run: `pnpm vitest run` → 全部通过（avatar 4 + commands 1 + logs 3）。

- [ ] **Step 6: 提交**

```bash
git add src/api/commands.ts src/stores/ui.ts src/stores/logs.ts src/__tests__/commands.spec.ts src/__tests__/logs.spec.ts
git commit -m "feat(ui): 类型化命令包装与 ui/logs store（事件监听）"
```

---

### Task 3: roots / apps / buckets store

**Files:**
- Create: `src/stores/roots.ts`、`src/stores/apps.ts`、`src/stores/buckets.ts`、`src/__tests__/stores.spec.ts`

**Interfaces:**
- Consumes: Task 2 的 commands
- Produces:
  - `useRootsStore`：state `roots: RootInfo[]`、`active: RootInfo | null`、`loading`、`error: string | null`；actions `load()`、`setActive(path)`、`add(path)`
  - `useAppsStore`：state `apps: InstalledApp[]`、`selectedName: string | null`、`loading`、`error`；getter `selected`；actions `load()`、`select(name)`
  - `useBucketsStore`：state `buckets: BucketInfo[]`、`loading`、`error`；actions `load()`

- [ ] **Step 1: 写失败测试**

`src/__tests__/stores.spec.ts`：

```ts
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  detectRoots: vi.fn(),
  getActiveRoot: vi.fn(),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
  listInstalledApps: vi.fn(),
  listBuckets: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

import { useAppsStore } from "../stores/apps";
import { useBucketsStore } from "../stores/buckets";
import { useRootsStore } from "../stores/roots";

const rootFixture = {
  path: "C:\\scoop",
  valid: true,
  source: "default",
  is_active: true,
  app_count: 9,
  bucket_count: 4,
  has_portable_config: false,
  global_dir: null,
};

const appFixture = {
  name: "git",
  version: "2.55.0.5",
  bucket: "main",
  architecture: "64bit",
  hold: false,
  updated: null,
  description: "git",
  homepage: "https://gitforwindows.org",
  icon: null,
  depends: [],
  persist: [],
  bin: ["git"],
  app_dir: "C:\\scoop\\apps\\git",
};

describe("stores", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("roots.load 同时拉取 active 与候选", async () => {
    mocks.getActiveRoot.mockResolvedValue(rootFixture);
    mocks.detectRoots.mockResolvedValue([rootFixture]);
    const store = useRootsStore();
    await store.load();
    expect(store.active?.path).toBe("C:\\scoop");
    expect(store.roots).toHaveLength(1);
    expect(store.loading).toBe(false);
  });

  it("roots.load 失败时 error 为可读字符串", async () => {
    mocks.getActiveRoot.mockRejectedValue("未找到可用的 scoop 根目录");
    mocks.detectRoots.mockResolvedValue([]);
    const store = useRootsStore();
    await store.load();
    expect(store.error).toBe("未找到可用的 scoop 根目录");
    expect(store.active).toBeNull();
  });

  it("roots.setActive 调用命令并刷新", async () => {
    mocks.setActiveRoot.mockResolvedValue(rootFixture);
    mocks.getActiveRoot.mockResolvedValue(rootFixture);
    mocks.detectRoots.mockResolvedValue([rootFixture]);
    const store = useRootsStore();
    await store.setActive("C:\\scoop");
    expect(mocks.setActiveRoot).toHaveBeenCalledWith("C:\\scoop");
    expect(store.active?.path).toBe("C:\\scoop");
  });

  it("apps.load 拉取列表并支持选中", async () => {
    mocks.listInstalledApps.mockResolvedValue([appFixture]);
    const store = useAppsStore();
    await store.load();
    store.select("git");
    expect(store.apps).toHaveLength(1);
    expect(store.selected?.name).toBe("git");
  });

  it("apps.load 失败时 error 与空列表", async () => {
    mocks.listInstalledApps.mockRejectedValue("未找到可用的 scoop 根目录");
    const store = useAppsStore();
    await store.load();
    expect(store.apps).toEqual([]);
    expect(store.error).toContain("未找到");
  });

  it("buckets.load 拉取列表", async () => {
    mocks.listBuckets.mockResolvedValue([{ name: "main", source: "https://x", is_git: true, manifest_count: 1641 }]);
    const store = useBucketsStore();
    await store.load();
    expect(store.buckets[0]).toMatchObject({ name: "main", manifest_count: 1641 });
  });
});
```

Run: `pnpm vitest run src/__tests__/stores.spec.ts` → 失败。

- [ ] **Step 2: 实现三个 store**

`src/stores/roots.ts`：

```ts
import { defineStore } from "pinia";
import { addRoot, detectRoots, getActiveRoot, setActiveRoot } from "../api/commands";
import type { RootInfo } from "../api/types";

export const useRootsStore = defineStore("roots", {
  state: () => ({
    roots: [] as RootInfo[],
    active: null as RootInfo | null,
    loading: false,
    error: null as string | null,
  }),
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.active = await getActiveRoot();
        this.roots = await detectRoots();
      } catch (e) {
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    async setActive(path: string) {
      this.error = null;
      try {
        this.active = await setActiveRoot(path);
        await this.load();
      } catch (e) {
        this.error = String(e);
      }
    },
    async add(path: string) {
      this.error = null;
      try {
        await addRoot(path);
        await this.load();
      } catch (e) {
        this.error = String(e);
      }
    },
  },
});
```

`src/stores/apps.ts`：

```ts
import { defineStore } from "pinia";
import { listInstalledApps } from "../api/commands";
import type { InstalledApp } from "../api/types";

export const useAppsStore = defineStore("apps", {
  state: () => ({
    apps: [] as InstalledApp[],
    selectedName: null as string | null,
    loading: false,
    error: null as string | null,
  }),
  getters: {
    selected(state): InstalledApp | null {
      return state.apps.find((a) => a.name === state.selectedName) ?? null;
    },
  },
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.apps = await listInstalledApps();
        if (!this.apps.some((a) => a.name === this.selectedName)) {
          this.selectedName = this.apps[0]?.name ?? null;
        }
      } catch (e) {
        this.apps = [];
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    select(name: string) {
      this.selectedName = name;
    },
  },
});
```

`src/stores/buckets.ts`：

```ts
import { defineStore } from "pinia";
import { listBuckets } from "../api/commands";
import type { BucketInfo } from "../api/types";

export const useBucketsStore = defineStore("buckets", {
  state: () => ({
    buckets: [] as BucketInfo[],
    loading: false,
    error: null as string | null,
  }),
  actions: {
    async load() {
      this.loading = true;
      this.error = null;
      try {
        this.buckets = await listBuckets();
      } catch (e) {
        this.buckets = [];
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
  },
});
```

- [ ] **Step 3: 运行测试确认通过**

Run: `pnpm vitest run` → 全部通过（新增 6 条）。

- [ ] **Step 4: 提交**

```bash
git add src/stores/roots.ts src/stores/apps.ts src/stores/buckets.ts src/__tests__/stores.spec.ts
git commit -m "feat(ui): roots/apps/buckets store 与测试"
```

---

### Task 4: 布局组件（AppShell / SideBar / TopBar / LogDrawer）+ App.vue 重写

**Files:**
- Create: `src/components/layout/AppShell.vue`、`src/components/layout/SideBar.vue`、`src/components/layout/TopBar.vue`、`src/components/layout/LogDrawer.vue`
- Modify: `src/App.vue`（重写为 AppShell + Naive UI Provider）
- Modify: `src-tauri/capabilities/default.json`（如需，为 opener 增加 http/https 作用域）
- Test: `src/__tests__/layout.spec.ts`

**Interfaces:**
- Consumes: Task 2/3 的 stores 与 commands
- Produces:
  - `AppShell.vue`：三栏骨架；`onMounted` 依次 `logs.init()` → `roots.load()` → `apps.load()` → `buckets.load()`
  - `SideBar.vue`：导航（8 项，用 Naive UI `n-menu`，label 中文）
  - `TopBar.vue`：根目录 `n-select`（选项 label = `路径（N 应用）`，value = path）、刷新、添加根目录（`@tauri-apps/plugin-dialog` 的 `open({ directory: true })`）、日志开关；`data-testid="topbar-root-select"` 便于测试
  - `LogDrawer.vue`：底部日志面板（默认收起，ui.logDrawerOpen 控制）；行按 stream 着色；任务运行时显示取消按钮（`cancelTask`）

- [ ] **Step 1: 实现四个布局组件与 App.vue**

`src/App.vue`：

```vue
<template>
  <n-config-provider :theme-overrides="themeOverrides">
    <n-message-provider>
      <AppShell />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { NConfigProvider, NMessageProvider } from "naive-ui";
import AppShell from "./components/layout/AppShell.vue";

const themeOverrides = {
  common: { primaryColor: "#2080f0", primaryColorHover: "#4098fc" },
};
</script>
```

`src/components/layout/AppShell.vue`：

```vue
<template>
  <div class="shell">
    <SideBar />
    <div class="main">
      <TopBar />
      <div class="content">
        <InstalledPage v-if="ui.activePage === 'installed'" />
        <BucketsPage v-else-if="ui.activePage === 'buckets'" />
        <SettingsPage v-else-if="ui.activePage === 'settings'" />
        <PlaceholderPage v-else :title="placeholderTitle" />
      </div>
      <LogDrawer />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from "vue";
import SideBar from "./SideBar.vue";
import TopBar from "./TopBar.vue";
import LogDrawer from "./LogDrawer.vue";
import InstalledPage from "../../pages/InstalledPage.vue";
import BucketsPage from "../../pages/BucketsPage.vue";
import SettingsPage from "../../pages/SettingsPage.vue";
import PlaceholderPage from "../../pages/PlaceholderPage.vue";
import { useUiStore } from "../../stores/ui";
import { useLogsStore } from "../../stores/logs";
import { useRootsStore } from "../../stores/roots";
import { useAppsStore } from "../../stores/apps";
import { useBucketsStore } from "../../stores/buckets";

const ui = useUiStore();
const placeholders: Record<string, string> = {
  store: "商店（M2：清单索引与搜索）",
  maintenance: "维护（M3：缓存清理与维护操作）",
  migrate: "环境迁移（M6：注册表扫描与接管）",
  profiles: "配置集（M5：Profile 切换）",
  backup: "备份（M5：快照与恢复）",
};
const placeholderTitle = computed(() => placeholders[ui.activePage] ?? "开发中");

onMounted(async () => {
  const logs = useLogsStore();
  await logs.init();
  await useRootsStore().load();
  await useAppsStore().load();
  await useBucketsStore().load();
});
</script>

<style scoped>
.shell {
  display: flex;
  height: 100vh;
  overflow: hidden;
  font-family: system-ui, "Microsoft YaHei", sans-serif;
}
.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
</style>
```

`src/components/layout/SideBar.vue`：

```vue
<template>
  <div class="sidebar">
    <div class="brand">ScoopUI</div>
    <n-menu :value="ui.activePage" :options="options" @update:value="onSelect" />
  </div>
</template>

<script setup lang="ts">
import { NMenu } from "naive-ui";
import type { MenuOption } from "naive-ui";
import { useUiStore } from "../../stores/ui";
import type { PageId } from "../../stores/ui";

const ui = useUiStore();

const options: MenuOption[] = [
  { label: "已安装", key: "installed" },
  { label: "商店", key: "store" },
  { label: "桶管理", key: "buckets" },
  { label: "维护", key: "maintenance" },
  { label: "环境迁移", key: "migrate" },
  { label: "配置集", key: "profiles" },
  { label: "备份", key: "backup" },
  { label: "设置", key: "settings" },
];

function onSelect(key: string) {
  ui.setPage(key as PageId);
}
</script>

<style scoped>
.sidebar {
  width: 180px;
  border-right: 1px solid #efeff5;
  display: flex;
  flex-direction: column;
}
.brand {
  padding: 16px;
  font-weight: 600;
  font-size: 16px;
}
</style>
```

`src/components/layout/TopBar.vue`：

```vue
<template>
  <div class="topbar">
    <div class="left">
      <span class="label">根目录</span>
      <n-select
        data-testid="topbar-root-select"
        style="width: 340px"
        size="small"
        :value="roots.active?.path ?? null"
        :options="rootOptions"
        :loading="roots.loading"
        placeholder="未找到 scoop 根目录"
        @update:value="onSelectRoot"
      />
      <n-tag v-if="roots.active && !roots.active.valid" type="error" size="small">无效</n-tag>
      <n-tag v-if="roots.active?.has_portable_config" size="small" type="warning">便携配置</n-tag>
    </div>
    <div class="right">
      <n-button size="small" :loading="anyLoading" @click="refresh">刷新</n-button>
      <n-button size="small" @click="pickRoot">添加根目录</n-button>
      <n-button size="small" :type="ui.logDrawerOpen ? 'primary' : 'default'" @click="ui.toggleLogs()">
        日志
      </n-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { NButton, NSelect, NTag, useMessage } from "naive-ui";
import { open } from "@tauri-apps/plugin-dialog";
import { useRootsStore } from "../../stores/roots";
import { useAppsStore } from "../../stores/apps";
import { useBucketsStore } from "../../stores/buckets";
import { useUiStore } from "../../stores/ui";

const roots = useRootsStore();
const apps = useAppsStore();
const buckets = useBucketsStore();
const ui = useUiStore();
const message = useMessage();

const rootOptions = computed(() =>
  roots.roots.map((r) => ({
    label: `${r.path}（${r.app_count} 应用）${r.is_active ? " · 当前" : ""}`,
    value: r.path,
  })),
);
const anyLoading = computed(() => roots.loading || apps.loading || buckets.loading);

async function onSelectRoot(path: string) {
  await roots.setActive(path);
  if (roots.error) {
    message.error(roots.error);
    return;
  }
  await Promise.all([apps.load(), buckets.load()]);
}

async function refresh() {
  await Promise.all([roots.load(), apps.load(), buckets.load()]);
  if (roots.error) message.error(roots.error);
}

async function pickRoot() {
  const dir = await open({ directory: true, title: "选择 scoop 根目录" });
  if (typeof dir !== "string") return;
  await roots.add(dir);
  if (roots.error) message.error(roots.error);
  else message.success("已添加根目录");
}
</script>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid #efeff5;
  gap: 12px;
}
.left,
.right {
  display: flex;
  align-items: center;
  gap: 8px;
}
.label {
  color: #666;
  font-size: 13px;
}
</style>
```

`src/components/layout/LogDrawer.vue`：

```vue
<template>
  <div v-if="ui.logDrawerOpen" class="drawer">
    <div class="header">
      <span class="title">任务日志</span>
      <span v-if="runningCount > 0" class="running">{{ runningCount }} 个任务运行中</span>
      <div class="spacer" />
      <n-button v-if="lastRunningTask" size="tiny" type="error" @click="cancelLast">取消任务</n-button>
      <n-button size="tiny" @click="logs.clear()">清空</n-button>
      <n-button size="tiny" @click="ui.toggleLogs()">收起</n-button>
    </div>
    <div ref="bodyEl" class="body">
      <div v-if="logs.lines.length === 0" class="empty">暂无日志</div>
      <div v-for="(l, i) in logs.lines" :key="i" class="line" :class="l.stream">
        {{ l.line }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { NButton } from "naive-ui";
import { useLogsStore } from "../../stores/logs";
import { useUiStore } from "../../stores/ui";
import { cancelTask } from "../../api/commands";

const logs = useLogsStore();
const ui = useUiStore();
const bodyEl = ref<HTMLElement | null>(null);

const runningCount = computed(() => Object.values(logs.running).filter(Boolean).length);
const lastRunningTask = computed(() => Object.keys(logs.running).find((k) => logs.running[k]) ?? null);

watch(
  () => logs.lines.length,
  async () => {
    await nextTick();
    if (bodyEl.value) bodyEl.value.scrollTop = bodyEl.value.scrollHeight;
  },
);

async function cancelLast() {
  if (lastRunningTask.value) await cancelTask(lastRunningTask.value);
}
</script>

<style scoped>
.drawer {
  height: 200px;
  border-top: 1px solid #efeff5;
  display: flex;
  flex-direction: column;
  background: #fafafc;
}
.header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-bottom: 1px solid #efeff5;
}
.title {
  font-weight: 600;
  font-size: 13px;
}
.running {
  color: #2080f0;
  font-size: 12px;
}
.spacer {
  flex: 1;
}
.body {
  flex: 1;
  overflow: auto;
  padding: 8px 10px;
  font-family: Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
}
.line.stderr {
  color: #d03050;
}
.line.meta {
  color: #18a058;
}
.empty {
  color: #999;
}
</style>
```

- [ ] **Step 2: 写布局冒烟测试**

`src/__tests__/layout.spec.ts`：

```ts
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  detectRoots: vi.fn().mockResolvedValue([]),
  getActiveRoot: vi.fn().mockResolvedValue(null),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
  listInstalledApps: vi.fn().mockResolvedValue([]),
  listBuckets: vi.fn().mockResolvedValue([]),
  getScoopConfig: vi.fn().mockResolvedValue(null),
  runScoop: vi.fn(),
  cancelTask: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import SideBar from "../components/layout/SideBar.vue";
import { useUiStore } from "../stores/ui";

describe("布局", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("侧边栏包含 8 个导航项且点击切换页面", async () => {
    const wrapper = mount(SideBar, {
      global: { stubs: { NMenu: { template: "<div><slot v-for='o in options' /></div>", props: ["options"] } } },
    });
    const text = wrapper.text();
    for (const label of ["已安装", "商店", "桶管理", "维护", "环境迁移", "配置集", "备份", "设置"]) {
      expect(text).toContain(label);
    }
    const ui = useUiStore();
    ui.setPage("settings");
    expect(ui.activePage).toBe("settings");
  });
});
```

注：`n-menu` 在 jsdom 下的完整渲染较重；该测试用 stub 简化，只断言导航项存在与 store 行为。完整的视觉验证在第 6 个任务用真实窗口截图完成。

Run: `pnpm vitest run src/__tests__/layout.spec.ts` → 通过（先跑一次确认初始失败：组件不存在）。

- [ ] **Step 3: opener 权限与打开主页/目录**

若 `src-tauri/capabilities/default.json` 的 `opener:default` 在运行时拒绝打开任意 https 链接，则替换为显式作用域：

```json
{
  "identifier": "opener:allow-open-url",
  "allow": [{ "url": "http://*" }, { "url": "https://*" }]
}
```

（`opener:default` 保留其余能力；修改后 `cargo check` 与 `pnpm tauri dev` 需通过。若默认权限已可打开任意 http/https，保持现状即可，并在报告中说明。）

- [ ] **Step 4: 构建与测试**

Run: `pnpm test && pnpm build` → 全绿。

- [ ] **Step 5: 提交**

```bash
git add src/App.vue src/components/layout src/__tests__/layout.spec.ts src-tauri/capabilities/default.json
git commit -m "feat(ui): 三栏骨架（侧边栏/顶栏/日志抽屉）与布局测试"
```

---

### Task 5: 应用组件与页面（AppIcon / AppList / AppDetail / 四个页面）

**Files:**
- Create: `src/components/app/AppIcon.vue`、`src/components/app/AppList.vue`、`src/components/app/AppDetail.vue`
- Create: `src/pages/InstalledPage.vue`、`src/pages/BucketsPage.vue`、`src/pages/SettingsPage.vue`、`src/pages/PlaceholderPage.vue`
- Test: `src/__tests__/app-components.spec.ts`

**Interfaces:**
- Consumes: Task 1-3 的 types/stores
- Produces:
  - `AppIcon.vue`：props `{ name: string; size?: number }`；渲染首字母头像（背景色 = `avatarColor`）
  - `AppList.vue`：props `{ apps: InstalledApp[]; selected: string | null }`；emit `select(name)`；行显示 名称/版本/桶/hold 标签
  - `AppDetail.vue`：props `{ app: InstalledApp | null }`；显示大图标、名称版本、描述、主页（按钮 → `openUrl`）、依赖、persist、安装目录（按钮 → `openPath`）；无选中显示空态
  - `InstalledPage.vue`：列表 + 详情两栏
  - `BucketsPage.vue`：`n-table` 显示 名称/来源/清单数/类型
  - `SettingsPage.vue`：活动根信息、配置文件路径与 portable 标记、配置 JSON（`<pre>`）、"检查环境"按钮（`runScoop(["--version"], taskId)` + 打开日志抽屉）
  - `PlaceholderPage.vue`：props `{ title: string }`

- [ ] **Step 1: 写失败测试（AppIcon / AppList / AppDetail）**

`src/__tests__/app-components.spec.ts`：

```ts
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AppDetail from "../components/app/AppDetail.vue";
import AppIcon from "../components/app/AppIcon.vue";
import AppList from "../components/app/AppList.vue";
import type { InstalledApp } from "../api/types";

const git: InstalledApp = {
  name: "git",
  version: "2.55.0.5",
  bucket: "main",
  architecture: "64bit",
  hold: false,
  updated: "2026-03-25T10:16:52Z",
  description: "分布式版本控制系统",
  homepage: "https://gitforwindows.org",
  icon: null,
  depends: ["less"],
  persist: ["etc/gitconfig"],
  bin: ["git"],
  app_dir: "C:\\scoop\\apps\\git",
};

describe("AppIcon", () => {
  it("无图标时显示首字母与稳定背景色", () => {
    const wrapper = mount(AppIcon, { props: { name: "git" } });
    expect(wrapper.text()).toContain("G");
    const style = wrapper.find(".icon").attributes("style") ?? "";
    expect(style).toContain("background");
  });
});

describe("AppList", () => {
  it("渲染应用行并点击 emit select", async () => {
    const wrapper = mount(AppList, { props: { apps: [git], selected: null } });
    expect(wrapper.text()).toContain("git");
    expect(wrapper.text()).toContain("2.55.0.5");
    await wrapper.find(".row").trigger("click");
    expect(wrapper.emitted("select")?.[0]).toEqual(["git"]);
  });
});

describe("AppDetail", () => {
  it("渲染详情字段", () => {
    const wrapper = mount(AppDetail, { props: { app: git } });
    const text = wrapper.text();
    expect(text).toContain("2.55.0.5");
    expect(text).toContain("分布式版本控制系统");
    expect(text).toContain("less");
    expect(text).toContain("etc/gitconfig");
  });

  it("空选中显示空态", () => {
    const wrapper = mount(AppDetail, { props: { app: null } });
    expect(wrapper.text()).toContain("选择一个应用");
  });
});
```

Run: `pnpm vitest run src/__tests__/app-components.spec.ts` → 失败（组件不存在）。

- [ ] **Step 2: 实现 AppIcon / AppList / AppDetail**

`src/components/app/AppIcon.vue`：

```vue
<template>
  <div
    class="icon"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      background: avatarColor(name),
      fontSize: `${Math.round(size * 0.45)}px`,
      borderRadius: `${Math.round(size * 0.2)}px`,
    }"
  >
    {{ avatarLetter(name) }}
  </div>
</template>

<script setup lang="ts">
import { avatarColor, avatarLetter } from "../../utils/avatar";

withDefaults(defineProps<{ name: string; size?: number }>(), { size: 24 });
</script>

<style scoped>
.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 600;
  flex-shrink: 0;
}
</style>
```

`src/components/app/AppList.vue`：

```vue
<template>
  <div class="list">
    <div v-if="apps.length === 0" class="empty">没有已安装的应用</div>
    <div
      v-for="app in apps"
      :key="app.name"
      class="row"
      :class="{ active: app.name === selected }"
      @click="$emit('select', app.name)"
    >
      <AppIcon :name="app.name" :size="28" />
      <div class="info">
        <div class="name">
          {{ app.name }}
          <span v-if="app.hold" class="hold">锁定</span>
        </div>
        <div class="meta">
          {{ app.version }}
          <span v-if="app.bucket"> · {{ app.bucket }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "./AppIcon.vue";
import type { InstalledApp } from "../../api/types";

defineProps<{ apps: InstalledApp[]; selected: string | null }>();
defineEmits<{ select: [name: string] }>();
</script>

<style scoped>
.list {
  overflow: auto;
  height: 100%;
  border-right: 1px solid #efeff5;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
}
.row:hover {
  background: #f5f7fa;
}
.row.active {
  background: #eaf3ff;
}
.name {
  font-size: 14px;
}
.meta {
  font-size: 12px;
  color: #888;
}
.hold {
  margin-left: 6px;
  font-size: 11px;
  color: #f0a020;
  border: 1px solid #f0a020;
  border-radius: 3px;
  padding: 0 4px;
}
.empty {
  padding: 24px;
  color: #999;
  text-align: center;
}
</style>
```

`src/components/app/AppDetail.vue`：

```vue
<template>
  <div class="detail">
    <div v-if="!app" class="empty">选择一个应用查看详情</div>
    <template v-else>
      <div class="head">
        <AppIcon :name="app.name" :size="64" />
        <div>
          <div class="title">
            {{ app.name }}
            <n-tag v-if="app.hold" size="small" type="warning">锁定</n-tag>
          </div>
          <div class="sub">版本 {{ app.version }}<span v-if="app.bucket"> · 来自 {{ app.bucket }}</span><span v-if="app.architecture"> · {{ app.architecture }}</span></div>
        </div>
      </div>

      <p v-if="app.description" class="desc">{{ app.description }}</p>

      <div class="actions">
        <n-button v-if="app.homepage" size="small" @click="openHomepage">打开主页</n-button>
        <n-button size="small" @click="openDir">打开安装目录</n-button>
      </div>

      <n-descriptions :column="1" size="small" label-placement="left" class="fields">
        <n-descriptions-item v-if="app.updated" label="更新时间">{{ app.updated }}</n-descriptions-item>
        <n-descriptions-item label="安装目录">{{ app.app_dir }}</n-descriptions-item>
      </n-descriptions>

      <div v-if="app.depends.length" class="section">
        <div class="section-title">依赖</div>
        <n-tag v-for="d in app.depends" :key="d" size="small" class="tag">{{ d }}</n-tag>
      </div>

      <div v-if="app.persist.length" class="section">
        <div class="section-title">持久化数据（persist）</div>
        <div v-for="p in app.persist" :key="p" class="persist">{{ p }}</div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { NButton, NDescriptions, NDescriptionsItem, NTag } from "naive-ui";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "./AppIcon.vue";
import type { InstalledApp } from "../../api/types";

const props = defineProps<{ app: InstalledApp | null }>();

async function openHomepage() {
  if (props.app?.homepage) await openUrl(props.app.homepage);
}

async function openDir() {
  if (props.app) await openPath(props.app.app_dir);
}
</script>

<style scoped>
.detail {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
.head {
  display: flex;
  gap: 14px;
  align-items: center;
}
.title {
  font-size: 18px;
  font-weight: 600;
  display: flex;
  gap: 8px;
  align-items: center;
}
.sub {
  color: #888;
  font-size: 13px;
  margin-top: 4px;
}
.desc {
  margin: 14px 0 0;
  color: #444;
}
.actions {
  display: flex;
  gap: 8px;
  margin: 14px 0;
}
.fields {
  margin-top: 6px;
}
.section {
  margin-top: 16px;
}
.section-title {
  font-size: 13px;
  color: #666;
  margin-bottom: 6px;
}
.tag {
  margin-right: 6px;
}
.persist {
  font-family: Consolas, monospace;
  font-size: 12px;
  color: #555;
}
.empty {
  color: #999;
  text-align: center;
  padding-top: 60px;
}
</style>
```

- [ ] **Step 3: 实现四个页面**

`src/pages/InstalledPage.vue`：

```vue
<template>
  <div class="page">
    <AppList :apps="apps.apps" :selected="apps.selectedName" class="list" @select="apps.select" />
    <AppDetail :app="apps.selected" class="detail" />
  </div>
</template>

<script setup lang="ts">
import AppList from "../components/app/AppList.vue";
import AppDetail from "../components/app/AppDetail.vue";
import { useAppsStore } from "../stores/apps";

const apps = useAppsStore();
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.list {
  width: 280px;
  flex-shrink: 0;
}
.detail {
  flex: 1;
  min-width: 0;
}
</style>
```

`src/pages/BucketsPage.vue`：

```vue
<template>
  <div class="page">
    <n-table :bordered="false" size="small">
      <thead>
        <tr>
          <th>名称</th>
          <th>来源</th>
          <th>清单数</th>
          <th>类型</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="b in store.buckets" :key="b.name">
          <td>{{ b.name }}</td>
          <td class="source">{{ b.source ?? "（本地目录，非 git）" }}</td>
          <td>{{ b.manifest_count }}</td>
          <td>{{ b.is_git ? "git" : "目录" }}</td>
        </tr>
      </tbody>
    </n-table>
    <div v-if="store.error" class="error">{{ store.error }}</div>
  </div>
</template>

<script setup lang="ts">
import { NTable } from "naive-ui";
import { useBucketsStore } from "../stores/buckets";

const store = useBucketsStore();
</script>

<style scoped>
.page {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
.source {
  font-family: Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}
.error {
  color: #d03050;
  margin-top: 12px;
}
</style>
```

`src/pages/SettingsPage.vue`：

```vue
<template>
  <div class="page">
    <h3>运行环境</h3>
    <n-descriptions :column="1" size="small" bordered>
      <n-descriptions-item label="当前根目录">
        {{ roots.active?.path ?? "未设置" }}
      </n-descriptions-item>
      <n-descriptions-item label="应用 / 桶数量">
        {{ roots.active ? `${roots.active.app_count} / ${roots.active.bucket_count}` : "-" }}
      </n-descriptions-item>
      <n-descriptions-item label="全局目录">
        {{ roots.active?.global_dir ?? "-" }}
      </n-descriptions-item>
    </n-descriptions>
    <div class="actions">
      <n-button size="small" :disabled="!roots.active" @click="checkEnv">检查环境（scoop --version）</n-button>
    </div>

    <h3>配置文件</h3>
    <n-descriptions v-if="config" :column="1" size="small" bordered>
      <n-descriptions-item label="路径">{{ config.file_path }}</n-descriptions-item>
      <n-descriptions-item label="便携模式">{{ config.portable ? "是" : "否" }}</n-descriptions-item>
    </n-descriptions>
    <pre v-if="config" class="json">{{ configJson }}</pre>
    <div v-else class="empty">暂无配置信息</div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { NButton, NDescriptions, NDescriptionsItem } from "naive-ui";
import { getScoopConfig, runScoop } from "../api/commands";
import type { ScoopConfig } from "../api/types";
import { useLogsStore } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import { useUiStore } from "../stores/ui";

const roots = useRootsStore();
const logs = useLogsStore();
const ui = useUiStore();
const config = ref<ScoopConfig | null>(null);

const configJson = computed(() => (config.value ? JSON.stringify(config.value.values, null, 2) : ""));

onMounted(async () => {
  if (!roots.active) return;
  try {
    config.value = await getScoopConfig();
  } catch {
    config.value = null;
  }
});

async function checkEnv() {
  const taskId = `check-${Date.now()}`;
  ui.logDrawerOpen = true;
  logs.start(taskId, "scoop --version");
  try {
    await runScoop(["--version"], taskId);
  } catch (e) {
    logs.lines.push({ taskId, stream: "stderr", line: String(e) });
    logs.end({ taskId, code: null, success: false });
  }
}
</script>

<style scoped>
.page {
  padding: 16px;
  overflow: auto;
  height: 100%;
}
h3 {
  margin: 18px 0 8px;
  font-size: 14px;
}
h3:first-child {
  margin-top: 0;
}
.actions {
  margin-top: 10px;
}
.json {
  margin-top: 10px;
  background: #f7f7f9;
  border-radius: 4px;
  padding: 10px;
  font-size: 12px;
  overflow: auto;
  max-height: 240px;
}
.empty {
  color: #999;
}
</style>
```

`src/pages/PlaceholderPage.vue`：

```vue
<template>
  <div class="page">{{ title }}</div>
</template>

<script setup lang="ts">
defineProps<{ title: string }>();
</script>

<style scoped>
.page {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #999;
}
</style>
```

- [ ] **Step 4: 运行测试与构建**

Run: `pnpm test && pnpm build` → 全绿（AppIcon/AppList/AppDetail 4 条新增）。

- [ ] **Step 5: 提交**

```bash
git add src/components/app src/pages src/__tests__/app-components.spec.ts
git commit -m "feat(ui): 应用列表/详情组件与四个页面（已安装/桶/设置/占位）"
```

---

### Task 6: 真实窗口验收（截图 + 手动清单）

**Files:**
- 无代码改动（验收任务；如发现问题回到对应任务修复）

**Interfaces:**
- Consumes: Task 1-5 全部
- Produces: 窗口截图证据（`%TEMP%\opencode\scoopui-m1b.png`）+ 报告

- [ ] **Step 1: 启动 dev 并等待窗口**

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
nohup pnpm tauri dev > "/c/Users/Administrator/AppData/Local/Temp/opencode/tauri-dev-m1b.log" 2>&1 &
for i in $(seq 1 60); do
  T=$(powershell -NoProfile -Command "(Get-Process scoopui -ErrorAction SilentlyContinue | Select-Object -First 1).MainWindowTitle" 2>/dev/null)
  if [ -n "$T" ]; then echo "WINDOW_TITLE=$T"; break; fi
  sleep 5
done
```

- [ ] **Step 2: 截图窗口**

```bash
powershell -NoProfile -Command '
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$p = Get-Process scoopui | Select-Object -First 1
[W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 800
$r = New-Object W+RECT
[W]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
$w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
$bmp.Save("C:\Users\Administrator\AppData\Local\Temp\opencode\scoopui-m1b.png", [System.Drawing.Imaging.ImageFormat]::Png)
"SAVED $w x $h"
'
```

Expected: 输出 `SAVED <w> x <h>`（窗口 1200x760 左右）。

- [ ] **Step 3: 验收判定（控制者将亲自查看截图）**

用 Read 工具打开截图，核对：侧边栏 8 项中文导航、顶栏根目录下拉显示 `C:\Users\Administrator\scoop（9 应用）`、已安装列表出现 9 个应用（7zip/android-clt/cmake/dark/flutter/git/mingw/python/temurin17-jdk）、右侧详情显示选中应用信息、无空白/报错红字。

- [ ] **Step 4: 关闭 dev**

```bash
taskkill.exe //F //IM scoopui.exe 2>&1 | head -2
```

- [ ] **Step 5: 写入验收报告**

报告：`C:\Users\Administrator\Desktop\idear\scoopUI\.superpowers\sdd\task-m1b-acceptance.md`，含：截图路径、窗口尺寸、界面要素清单逐项核对结果、发现的问题（如有，附修复建议）。

注：本任务不提交代码（无改动）。

---

### Task 7: M1b 里程碑评审

**Files:**
- 无代码改动（评审任务）

**Interfaces:**
- Consumes: Task 1-6 全部产物
- Produces: 评审报告（控制者按报告决定是否需要修复波）

- [ ] **Step 1: 生成评审包**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
SKILL_DIR="/c/Users/Administrator/.cache/opencode/packages/superpowers@git+https_/github.com/jnlk-cn/superpowers-cn.git/node_modules/superpowers/skills/subagent-driven-development"
bash "$SKILL_DIR/scripts/review-package" <M1b_BASE> HEAD
```

- [ ] **Step 2: 分派评审子代理（读源码方式，勿用大 diff）**

评审范围：`src/api/*.ts`、`src/stores/*.ts`、`src/components/**/*.vue`、`src/pages/*.vue`、`src/utils/avatar.ts`、`src/__tests__/*.spec.ts`、`vite.config.ts`、`src-tauri/capabilities/default.json`。
评审重点：契约对齐（snake_case/camelCase/null/中文错误）、事件监听时机、store 错误处理、组件职责边界、测试真实性、是否存在 M2 越界（商店/图标/更新检测不得提前实现）。

---

## M1b 验收清单（人工）

- [ ] `pnpm test` 全绿（预计 avatar 4 + commands 1 + logs 3 + stores 6 + layout 1 + app-components 4 ≈ 19 条）
- [ ] `pnpm build` 无类型错误
- [ ] `pnpm tauri dev` 窗口标题 `ScoopUI`，截图显示三栏布局与 9 个应用
- [ ] 顶栏根目录下拉可切换并触发列表刷新（人工点选验证）
- [ ] 设置页"检查环境"按钮触发日志抽屉实时输出 `scoop --version` 内容（人工点击验证）
- [ ] 打开主页/打开安装目录按钮工作（人工点击验证；若权限报错，按 Task 4 Step 3 调整作用域）

## Self-Review 记录

- 规格覆盖：设计文档 §5（三栏主框架与页面结构）+ M1 验收（列出 9 应用、实时日志）全部对应到 Task 1-6；M2 的图标/搜索/更新检测未提前实现（AppIcon 只做首字母兜底，`icon` 字段预留）。
- 契约一致性：types.ts 字段与 M1a 序列化逐字一致（snake_case）；事件 payload camelCase；`run_scoop` 参数名 `args`/`taskId` 与 Tauri 默认转换一致。
- 占位符扫描：无 TBD；每个组件/页面均有完整代码。
- 已知取舍：SideBar 测试用 stub 简化（完整渲染由 Task 6 截图验证）；opener 权限作用域按运行时结果二选一（已给出两条路径）。
