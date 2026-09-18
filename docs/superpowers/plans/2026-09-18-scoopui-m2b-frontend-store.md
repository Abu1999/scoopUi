# ScoopUI M2b（前端：商店页搜索 + 图标展示）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 商店页可用：搜索框（300ms 防抖）实时搜索 M2a 离线索引、结果列表/卡片网格切换、包详情面板；已装列表与详情接入真实图标（manifest.icon → homepage favicon → 首字母兜底），前端记忆化 + 会话负缓存 + 并发上限 6。

**Architecture:** `src/api/commands.ts` 增加 3 个 M2a 命令包装 → `src/stores/icons.ts`（source→dataURL 记忆化、去重、并发队列 6、失败负缓存）与 `src/stores/packages.ts`（搜索/选中/竞态序号/刷新索引）→ `AppIcon.vue` 增加 `source` 分支 → `src/pages/StorePage.vue` + `src/components/store/PackageDetail.vue` → AppShell 用 StorePage 替换商店占位；AppList/AppDetail 传 `icon ?? homepage`。后端无改动（M2a 已交付）。

**Tech Stack:** Vue 3.5 + TypeScript + Vite + Naive UI + Pinia + Vitest（jsdom）。无新增依赖。

## Global Constraints

- **契约对齐（M2a 已冻结）**：
  - `PackageEntry { name, bucket, version, description, homepage, license, icon, deprecated }` —— 全 snake_case，TS 接口逐字照抄
  - `search_packages(query: String, limit: Option<usize>) -> Vec<PackageEntry>`；前端不传 limit 时后端默认 50
  - `refresh_package_index() -> usize`（条目数）；`fetch_icon(source) -> Option<String>`（`data:<mime>;base64,...`；失败 null）
  - 命令 reject 为中文 string（`String(e)` 即用户可读信息）
- 前端只能经 `src/api/commands.ts` 调后端；不得直接 import `@tauri-apps/api/core`（`src/api/tauri.ts` 除外）
- 图标：`source = manifest.icon ?? homepage`；抓取失败静默回退首字母；会话内负缓存不重试；并发上限 6（设计 §4.4）
- UI 文案全中文；代码不加注释；不加新依赖
- 组件测试挂载含 `AppIcon` 的组件时必须有 active pinia（`AppIcon` setup 会 `useIconsStore()`）
- 验证门：`pnpm test` 全绿、`pnpm build`（vue-tsc）无错、`pnpm tauri dev` 截图验收（Task 6）
- 初始 BASE 提交：`8f58810`

---

### Task 1: PackageEntry 类型 + 3 个命令包装

**Files:**
- Modify: `src/api/types.ts`（追加 `PackageEntry`）
- Modify: `src/api/commands.ts`（追加 3 个包装）
- Test: `src/__tests__/commands.spec.ts`（扩展契约断言）

**Interfaces:**
- Consumes: `src/api/tauri.ts` 的 `invoke`
- Produces:
  - `PackageEntry`（`src/api/types.ts`，Task 4/5 引用）
  - `searchPackages(query: string, limit?: number): Promise<PackageEntry[]>`
  - `refreshPackageIndex(): Promise<number>`
  - `fetchIcon(source: string): Promise<string | null>`

- [ ] **Step 1: 写失败测试（扩展 commands.spec.ts）**

把 `src/__tests__/commands.spec.ts` 全文替换为：

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  addRoot,
  cancelTask,
  detectRoots,
  fetchIcon,
  getActiveRoot,
  getScoopConfig,
  listBuckets,
  listInstalledApps,
  refreshPackageIndex,
  runScoop,
  searchPackages,
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

  it("M2 命令名与参数符合后端契约", async () => {
    await searchPackages("git");
    expect(invokeMock).toHaveBeenLastCalledWith("search_packages", { query: "git", limit: undefined });

    await searchPackages("git", 20);
    expect(invokeMock).toHaveBeenLastCalledWith("search_packages", { query: "git", limit: 20 });

    await refreshPackageIndex();
    expect(invokeMock).toHaveBeenLastCalledWith("refresh_package_index", undefined);

    await fetchIcon("https://gitforwindows.org");
    expect(invokeMock).toHaveBeenLastCalledWith("fetch_icon", { source: "https://gitforwindows.org" });
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/commands.spec.ts`
Expected: FAIL —— `searchPackages` 等导出不存在（或 `PackageEntry` 类型不存在导致 vue-tsc 报错；vitest 运行时报 import undefined）。

- [ ] **Step 3: 实现类型与包装**

`src/api/types.ts` 末尾追加：

```ts
export interface PackageEntry {
  name: string;
  bucket: string;
  version: string;
  description: string | null;
  homepage: string | null;
  license: string | null;
  icon: string | null;
  deprecated: boolean;
}
```

`src/api/commands.ts` 改为：

```ts
import type {
  BucketInfo,
  InstalledApp,
  PackageEntry,
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
export const searchPackages = (query: string, limit?: number) =>
  invoke<PackageEntry[]>("search_packages", { query, limit });
export const refreshPackageIndex = () => invoke<number>("refresh_package_index");
export const fetchIcon = (source: string) => invoke<string | null>("fetch_icon", { source });
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/commands.spec.ts`
Expected: PASS（2 tests）。

- [ ] **Step 5: 类型检查**

Run: `pnpm build`
Expected: 通过（无 vue-tsc 错误）。

- [ ] **Step 6: 提交**

```bash
git add src/api/types.ts src/api/commands.ts src/__tests__/commands.spec.ts
git commit -m "feat(ui): PackageEntry 类型与搜索/索引刷新/图标命令包装"
```

---

### Task 2: icons store（记忆化 / 去重 / 并发 6 / 会话负缓存）

**Files:**
- Create: `src/stores/icons.ts`
- Test: `src/__tests__/icons.spec.ts`

**Interfaces:**
- Consumes: Task 1 的 `fetchIcon`
- Produces（Task 3/5 引用）:
  - `useIconsStore()`:
    - state：`map: Record<string, string>`（source→dataURL）、`failed: Record<string, true>`、`pending: Record<string, true>`、`queue: string[]`、`active: number`
    - action `resolve(source: string | null | undefined): string | null`（命中 map 返回值，否则 null）
    - action `ensure(source: string | null | undefined): void`（空 source / 已成功 / 已失败 / 飞行中均直接返回；否则入队并启动泵）
    - action `pump(): void`（`active < 6` 时出队调用 `fetchIcon`；成功写 `map`，null/异常写 `failed`；完成后 `active--` 并继续泵）

- [ ] **Step 1: 写失败测试**

创建 `src/__tests__/icons.spec.ts`：

```ts
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ fetchIcon: vi.fn() }));
vi.mock("../api/commands", () => mocks);

import { useIconsStore } from "../stores/icons";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("icons store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("成功结果按 source 记忆化，重复 ensure 只抓一次", async () => {
    mocks.fetchIcon.mockResolvedValue("data:image/png;base64,AA==");
    const store = useIconsStore();
    store.ensure("https://x.test/favicon.ico");
    store.ensure("https://x.test/favicon.ico");
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(1);
    await flushPromises();
    expect(store.resolve("https://x.test/favicon.ico")).toBe("data:image/png;base64,AA==");
    store.ensure("https://x.test/favicon.ico");
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(1);
  });

  it("fetchIcon 返回 null 进入会话负缓存，不再重试", async () => {
    mocks.fetchIcon.mockResolvedValue(null);
    const store = useIconsStore();
    store.ensure("https://y.test/favicon.ico");
    await flushPromises();
    expect(store.resolve("https://y.test/favicon.ico")).toBeNull();
    store.ensure("https://y.test/favicon.ico");
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(1);
  });

  it("抓取拒绝也进入负缓存", async () => {
    mocks.fetchIcon.mockRejectedValue(new Error("boom"));
    const store = useIconsStore();
    store.ensure("https://z.test/favicon.ico");
    await flushPromises();
    store.ensure("https://z.test/favicon.ico");
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(1);
  });

  it("空 source 直接忽略", () => {
    const store = useIconsStore();
    store.ensure(null);
    store.ensure(undefined);
    store.ensure("");
    expect(mocks.fetchIcon).not.toHaveBeenCalled();
  });

  it("并发上限 6，完成后启动队列中的下一个", async () => {
    const tasks = Array.from({ length: 7 }, () => deferred<string | null>());
    let calls = 0;
    mocks.fetchIcon.mockImplementation(() => tasks[calls++].promise);
    const store = useIconsStore();
    for (let i = 0; i < 7; i++) {
      store.ensure(`https://s${i}.test/favicon.ico`);
    }
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(6);
    tasks[0].resolve("data:image/png;base64,AA==");
    await flushPromises();
    expect(mocks.fetchIcon).toHaveBeenCalledTimes(7);
    tasks[1].resolve("data:image/png;base64,AA==");
    await flushPromises();
    expect(store.resolve("https://s1.test/favicon.ico")).toBe("data:image/png;base64,AA==");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/icons.spec.ts`
Expected: FAIL —— `../stores/icons` 模块不存在。

- [ ] **Step 3: 实现 icons store**

创建 `src/stores/icons.ts`：

```ts
import { defineStore } from "pinia";
import { fetchIcon } from "../api/commands";

const MAX_CONCURRENT = 6;

export const useIconsStore = defineStore("icons", {
  state: () => ({
    map: {} as Record<string, string>,
    failed: {} as Record<string, true>,
    pending: {} as Record<string, true>,
    queue: [] as string[],
    active: 0,
  }),
  actions: {
    resolve(source: string | null | undefined): string | null {
      if (!source) return null;
      return this.map[source] ?? null;
    },
    ensure(source: string | null | undefined) {
      if (!source || this.map[source] || this.failed[source] || this.pending[source]) return;
      this.pending[source] = true;
      this.queue.push(source);
      this.pump();
    },
    pump() {
      while (this.active < MAX_CONCURRENT && this.queue.length > 0) {
        const source = this.queue.shift()!;
        this.active += 1;
        fetchIcon(source)
          .then((url) => {
            if (url) {
              this.map[source] = url;
            } else {
              this.failed[source] = true;
            }
          })
          .catch(() => {
            this.failed[source] = true;
          })
          .finally(() => {
            delete this.pending[source];
            this.active -= 1;
            this.pump();
          });
      }
    },
  },
});
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/icons.spec.ts`
Expected: PASS（5 tests）。

- [ ] **Step 5: 提交**

```bash
git add src/stores/icons.ts src/__tests__/icons.spec.ts
git commit -m "feat(ui): 图标 store（记忆化/去重/并发 6/会话负缓存）"
```

---

### Task 3: AppIcon 真实图标分支 + 既有测试适配

**Files:**
- Modify: `src/components/app/AppIcon.vue`
- Test: `src/__tests__/app-components.spec.ts`（加 `fetchIcon` mock、active pinia、3 个新用例）

**Interfaces:**
- Consumes: Task 2 的 `useIconsStore`
- Produces: `AppIcon` 新 prop `source?: string | null`（默认 null）；有 source 且 store 命中 → `<img class="icon-img">`，否则首字母 div（结构不变）

- [ ] **Step 1: 写失败测试（替换 app-components.spec.ts 全文）**

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AppDetail from "../components/app/AppDetail.vue";
import AppIcon from "../components/app/AppIcon.vue";
import AppList from "../components/app/AppList.vue";
import InstalledPage from "../pages/InstalledPage.vue";
import SettingsPage from "../pages/SettingsPage.vue";
import { useAppsStore } from "../stores/apps";
import { useRootsStore } from "../stores/roots";
import type { InstalledApp, RootInfo } from "../api/types";

const messageError = vi.hoisted(() => vi.fn());
const fetchIcon = vi.hoisted(() => vi.fn());

const listInstalledApps = vi.hoisted(() => vi.fn());
const getScoopConfig = vi.hoisted(() => vi.fn());
const runScoop = vi.hoisted(() => vi.fn());
const otherCommands = vi.hoisted(() => ({
  detectRoots: vi.fn(),
  getActiveRoot: vi.fn(),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
  listBuckets: vi.fn(),
  cancelTask: vi.fn(),
}));

vi.mock("../api/commands", () => ({
  ...otherCommands,
  listInstalledApps,
  getScoopConfig,
  runScoop,
  fetchIcon,
}));

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      info: vi.fn(),
      success: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(() => Promise.reject(new Error("denied"))),
  openUrl: vi.fn(() => Promise.reject(new Error("denied"))),
}));

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

beforeEach(() => {
  setActivePinia(createPinia());
  fetchIcon.mockReset().mockResolvedValue(null);
});

describe("AppIcon", () => {
  it("无 source 时显示首字母与固定背景色，且不请求图标", () => {
    const wrapper = mount(AppIcon, { props: { name: "git" } });
    expect(wrapper.text()).toContain("G");
    const style = wrapper.find(".icon").attributes("style") ?? "";
    expect(style.includes("#f0a020") || style.includes("rgb(240, 160, 32)")).toBe(true);
    expect(fetchIcon).not.toHaveBeenCalled();
  });

  it("source 抓取成功后用 img 展示图标", async () => {
    fetchIcon.mockResolvedValue("data:image/png;base64,AA==");
    const wrapper = mount(AppIcon, {
      props: { name: "git", source: "https://gitforwindows.org" },
    });
    expect(fetchIcon).toHaveBeenCalledWith("https://gitforwindows.org");
    await flushPromises();
    const img = wrapper.find("img");
    expect(img.exists()).toBe(true);
    expect(img.attributes("src")).toBe("data:image/png;base64,AA==");
    expect(wrapper.find(".icon").exists()).toBe(false);
  });

  it("source 抓取失败时回退首字母", async () => {
    fetchIcon.mockResolvedValue(null);
    const wrapper = mount(AppIcon, {
      props: { name: "7zip", source: "https://7-zip.org" },
    });
    await flushPromises();
    expect(wrapper.find("img").exists()).toBe(false);
    expect(wrapper.text()).toContain("7");
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

  beforeEach(() => {
    messageError.mockClear();
  });

  it("打开安装目录失败时提示错误", async () => {
    const wrapper = mount(AppDetail, { props: { app: git } });
    const button = wrapper.findAll("button").find((b) => b.text().includes("打开安装目录"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("打开失败：无法打开目录");
  });

  it("打开主页失败时提示错误", async () => {
    const wrapper = mount(AppDetail, { props: { app: git } });
    const button = wrapper.findAll("button").find((b) => b.text().includes("打开主页"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("打开失败：无法打开链接");
  });
});

describe("InstalledPage", () => {
  it("加载失败时展示错误文本", async () => {
    listInstalledApps.mockRejectedValueOnce("未找到 scoop 根目录");
    const pinia = createPinia();
    setActivePinia(pinia);
    const apps = useAppsStore();
    await apps.load();
    const wrapper = mount(InstalledPage, { global: { plugins: [pinia] } });
    expect(wrapper.text()).toContain("未找到 scoop 根目录");
  });
});

describe("SettingsPage", () => {
  it("切换根目录后重新加载配置", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const roots = useRootsStore();
    const rootA: RootInfo = {
      path: "C:\\scoop",
      valid: true,
      source: "env",
      is_active: true,
      app_count: 1,
      bucket_count: 1,
      has_portable_config: false,
      global_dir: null,
    };
    roots.active = rootA;
    getScoopConfig.mockResolvedValueOnce({
      file_path: "C:\\scoop\\config.json",
      portable: false,
      values: {},
    });
    const wrapper = mount(SettingsPage, { global: { plugins: [pinia] } });
    await flushPromises();
    expect(getScoopConfig).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain("C:\\scoop\\config.json");
    roots.active = { ...rootA, path: "D:\\scoop", is_active: true };
    getScoopConfig.mockResolvedValueOnce({
      file_path: "D:\\scoop\\config.json",
      portable: true,
      values: {},
    });
    await flushPromises();
    expect(getScoopConfig).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain("D:\\scoop\\config.json");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/app-components.spec.ts`
Expected: FAIL —— AppIcon 不接受 `source` 等价于 `img` 不存在（"source 抓取成功后用 img 展示图标" 失败），且 `fetchIcon` 未被调用。

- [ ] **Step 3: 实现 AppIcon source 分支**

`src/components/app/AppIcon.vue` 全文替换为：

```vue
<template>
  <img
    v-if="url"
    class="icon-img"
    :src="url"
    :alt="name"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      borderRadius: `${Math.round(size * 0.2)}px`,
    }"
  />
  <div
    v-else
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
import { computed, onMounted, watch } from "vue";
import { avatarColor, avatarLetter } from "../../utils/avatar";
import { useIconsStore } from "../../stores/icons";

const props = withDefaults(
  defineProps<{ name: string; size?: number; source?: string | null }>(),
  { size: 24, source: null },
);

const icons = useIconsStore();
const url = computed(() => (props.source ? icons.resolve(props.source) : null));

onMounted(() => {
  if (props.source) icons.ensure(props.source);
});

watch(
  () => props.source,
  (source) => {
    if (source) icons.ensure(source);
  },
);
</script>

<style scoped>
.icon,
.icon-img {
  flex-shrink: 0;
}
.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  font-weight: 600;
}
.icon-img {
  display: inline-block;
  object-fit: contain;
}
</style>
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/app-components.spec.ts`
Expected: PASS（10 tests：AppIcon 3 + AppList 1 + AppDetail 4 + InstalledPage 1 + SettingsPage 1）。
再跑全量：`pnpm vitest run` → 全绿（既有 36 + icons 5 + 本任务净增 2）。

- [ ] **Step 5: 提交**

```bash
git add src/components/app/AppIcon.vue src/__tests__/app-components.spec.ts
git commit -m "feat(ui): AppIcon 接入真实图标（source 抓取 + 失败回退首字母）"
```

---

### Task 4: packages store（搜索 / 选中 / 竞态防护 / 刷新索引）

**Files:**
- Create: `src/stores/packages.ts`
- Test: `src/__tests__/packages.spec.ts`

**Interfaces:**
- Consumes: Task 1 的 `PackageEntry`、`searchPackages`、`refreshPackageIndex`
- Produces（Task 5 引用）:
  - `usePackagesStore()`:
    - state：`query: string`、`results: PackageEntry[]`、`selectedName: string | null`、`loading: boolean`、`refreshing: boolean`、`indexCount: number | null`、`error: string | null`、`seq: number`
    - getter `selected(): PackageEntry | null`
    - action `search(query: string): Promise<void>`（自增 seq；旧响应丢弃；结果变化且选中不在其中时回退第一个；失败 `error = String(e)` 且清空结果）
    - action `refresh(): Promise<void>`（`refreshPackageIndex()` → `indexCount` → 按当前 query 重搜）
    - action `select(name: string): void`

- [ ] **Step 1: 写失败测试**

创建 `src/__tests__/packages.spec.ts`：

```ts
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  searchPackages: vi.fn(),
  refreshPackageIndex: vi.fn(),
}));
vi.mock("../api/commands", () => mocks);

import type { PackageEntry } from "../api/types";
import { usePackagesStore } from "../stores/packages";

function pkg(name: string): PackageEntry {
  return {
    name,
    bucket: "main",
    version: "1.0",
    description: null,
    homepage: null,
    license: null,
    icon: null,
    deprecated: false,
  };
}

describe("packages store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("search 拉取结果并默认选中第一个", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git"), pkg("git-lfs")]);
    const store = usePackagesStore();
    await store.search("git");
    expect(mocks.searchPackages).toHaveBeenCalledWith("git");
    expect(store.results).toHaveLength(2);
    expect(store.selectedName).toBe("git");
    expect(store.selected?.name).toBe("git");
    expect(store.loading).toBe(false);
  });

  it("选中项在结果变化时回退到第一个", async () => {
    mocks.searchPackages.mockResolvedValueOnce([pkg("git"), pkg("git-lfs")]);
    const store = usePackagesStore();
    await store.search("git");
    store.select("git-lfs");
    mocks.searchPackages.mockResolvedValueOnce([pkg("git")]);
    await store.search("git");
    expect(store.selectedName).toBe("git");
  });

  it("慢的旧响应不会覆盖新结果", async () => {
    let resolveFirst!: (value: PackageEntry[]) => void;
    mocks.searchPackages.mockImplementationOnce(
      () =>
        new Promise<PackageEntry[]>((resolve) => {
          resolveFirst = resolve;
        }),
    );
    mocks.searchPackages.mockResolvedValueOnce([pkg("new")]);
    const store = usePackagesStore();
    const first = store.search("old");
    await store.search("new");
    resolveFirst([pkg("stale")]);
    await first;
    expect(store.query).toBe("new");
    expect(store.results.map((p) => p.name)).toEqual(["new"]);
  });

  it("search 失败写入 error 并清空结果", async () => {
    mocks.searchPackages.mockRejectedValue("未找到 scoop 根目录");
    const store = usePackagesStore();
    await store.search("git");
    expect(store.error).toBe("未找到 scoop 根目录");
    expect(store.results).toEqual([]);
  });

  it("refresh 重建索引后按当前查询重搜并记录数量", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git")]);
    const store = usePackagesStore();
    await store.search("git");
    mocks.refreshPackageIndex.mockResolvedValue(4955);
    await store.refresh();
    expect(mocks.refreshPackageIndex).toHaveBeenCalledTimes(1);
    expect(mocks.searchPackages).toHaveBeenCalledTimes(2);
    expect(mocks.searchPackages).toHaveBeenLastCalledWith("git");
    expect(store.indexCount).toBe(4955);
    expect(store.refreshing).toBe(false);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/packages.spec.ts`
Expected: FAIL —— `../stores/packages` 模块不存在。

- [ ] **Step 3: 实现 packages store**

创建 `src/stores/packages.ts`：

```ts
import { defineStore } from "pinia";
import { refreshPackageIndex, searchPackages } from "../api/commands";
import type { PackageEntry } from "../api/types";

export const usePackagesStore = defineStore("packages", {
  state: () => ({
    query: "",
    results: [] as PackageEntry[],
    selectedName: null as string | null,
    loading: false,
    refreshing: false,
    indexCount: null as number | null,
    error: null as string | null,
    seq: 0,
  }),
  getters: {
    selected(state): PackageEntry | null {
      return state.results.find((p) => p.name === state.selectedName) ?? null;
    },
  },
  actions: {
    async search(query: string) {
      this.query = query;
      const seq = ++this.seq;
      this.loading = true;
      this.error = null;
      try {
        const results = await searchPackages(query);
        if (seq !== this.seq) return;
        this.results = results;
        if (!results.some((p) => p.name === this.selectedName)) {
          this.selectedName = results[0]?.name ?? null;
        }
      } catch (e) {
        if (seq !== this.seq) return;
        this.results = [];
        this.selectedName = null;
        this.error = String(e);
      } finally {
        if (seq === this.seq) this.loading = false;
      }
    },
    async refresh() {
      this.refreshing = true;
      this.error = null;
      try {
        this.indexCount = await refreshPackageIndex();
        await this.search(this.query);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.refreshing = false;
      }
    },
    select(name: string) {
      this.selectedName = name;
    },
  },
});
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/packages.spec.ts`
Expected: PASS（5 tests）。

- [ ] **Step 5: 提交**

```bash
git add src/stores/packages.ts src/__tests__/packages.spec.ts
git commit -m "feat(ui): packages store（搜索/选中/竞态防护/刷新索引）"
```

---

### Task 5: StorePage + PackageDetail + AppShell/应用组件集成

**Files:**
- Create: `src/components/store/PackageDetail.vue`、`src/pages/StorePage.vue`
- Modify: `src/components/layout/AppShell.vue`（用 StorePage 替换商店占位）
- Modify: `src/components/app/AppList.vue`、`src/components/app/AppDetail.vue`（传 `icon ?? homepage`）
- Test: `src/__tests__/store-page.spec.ts`（新建）、`src/__tests__/appshell.spec.ts`（stub + 1 用例）、`src/__tests__/app-components.spec.ts`（+1 图标集成用例）

**Interfaces:**
- Consumes: Task 1 命令、Task 2/3 的 icons store 与 AppIcon `source`、Task 4 packages store
- Produces:
  - `PackageDetail.vue` props `{ pkg: PackageEntry | null }`；展示图标 64 / 名称 / 已弃用标签 / 版本 / 桶 / 许可证 / 描述 / 主页按钮；无安装按钮（M3）
  - `StorePage.vue`：搜索输入（300ms 防抖）、列表/卡片切换、结果区、右侧详情；挂载时按当前 query 搜索；根目录变化时按当前 query 重搜；刷新索引按钮 + 包数量

- [ ] **Step 1: 写失败测试（新建 store-page.spec.ts）**

创建 `src/__tests__/store-page.spec.ts`：

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const messageError = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  detectRoots: vi.fn().mockResolvedValue([]),
  getActiveRoot: vi.fn().mockResolvedValue(null),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
  listInstalledApps: vi.fn().mockResolvedValue([]),
  listBuckets: vi.fn().mockResolvedValue([]),
  getScoopConfig: vi.fn(),
  runScoop: vi.fn(),
  cancelTask: vi.fn(),
  searchPackages: vi.fn(),
  refreshPackageIndex: vi.fn(),
  fetchIcon: vi.fn().mockResolvedValue(null),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      info: vi.fn(),
      success: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
  openPath: vi.fn(() => Promise.resolve()),
}));

import StorePage from "../pages/StorePage.vue";
import type { PackageEntry } from "../api/types";
import { useRootsStore } from "../stores/roots";

function pkg(name: string, extra: Partial<PackageEntry> = {}): PackageEntry {
  return {
    name,
    bucket: "main",
    version: "1.0",
    description: null,
    homepage: null,
    license: null,
    icon: null,
    deprecated: false,
    ...extra,
  };
}

describe("StorePage", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.searchPackages.mockResolvedValue([
      pkg("git", { description: "分布式版本控制系统" }),
      pkg("git-lfs"),
    ]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("挂载时执行一次搜索并渲染结果与详情", async () => {
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledWith("");
    expect(wrapper.text()).toContain("git-lfs");
    expect(wrapper.find(".detail").text()).toContain("分布式版本控制系统");
  });

  it("输入防抖 300ms 后按新关键词搜索", async () => {
    vi.useFakeTimers();
    const wrapper = mount(StorePage);
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);

    await wrapper.find("input").setValue("git");
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(299);
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(mocks.searchPackages).toHaveBeenLastCalledWith("git");
  });

  it("可切换列表/卡片视图", async () => {
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(wrapper.find(".results.list").exists()).toBe(true);
    const gridButton = wrapper.findAll("button").find((b) => b.text().includes("卡片"));
    expect(gridButton).toBeTruthy();
    await gridButton!.trigger("click");
    expect(wrapper.find(".results.grid").exists()).toBe(true);
  });

  it("点击结果切换详情", async () => {
    const wrapper = mount(StorePage);
    await flushPromises();
    const rows = wrapper.findAll(".row");
    await rows[1].trigger("click");
    expect(wrapper.find(".detail").text()).toContain("git-lfs");
  });

  it("刷新索引后重搜并显示包数量", async () => {
    mocks.refreshPackageIndex.mockResolvedValue(4955);
    const wrapper = mount(StorePage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("刷新索引"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.refreshPackageIndex).toHaveBeenCalledTimes(1);
    expect(mocks.searchPackages).toHaveBeenCalledTimes(2);
    expect(wrapper.text()).toContain("4955");
  });

  it("根目录切换后按当前关键词重搜", async () => {
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledTimes(1);
    const roots = useRootsStore();
    roots.active = {
      path: "D:\\scoop",
      valid: true,
      source: "manual",
      is_active: true,
      app_count: 0,
      bucket_count: 0,
      has_portable_config: false,
      global_dir: null,
    };
    await flushPromises();
    expect(mocks.searchPackages).toHaveBeenCalledTimes(2);
  });

  it("搜索失败展示错误文本", async () => {
    mocks.searchPackages.mockRejectedValue("未找到 scoop 根目录");
    const wrapper = mount(StorePage);
    await flushPromises();
    expect(wrapper.text()).toContain("未找到 scoop 根目录");
  });
});
```

同时修改 `src/__tests__/appshell.spec.ts`：

（a）在 `h.commands` 对象中补 3 个 mock（`searchPackages: vi.fn(async () => [])`、`refreshPackageIndex: vi.fn(async () => 0)`、`fetchIcon: vi.fn(async () => null)`）；
（b）`stubs` 增加 `StorePage: true`；
（c）文件末尾追加用例：

```ts
  it("商店页渲染 StorePage 而非占位", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useUiStore().setPage("store");
    const wrapper = mount(AppShell, { global: { plugins: [pinia], stubs } });
    expect(wrapper.find("store-page-stub").exists()).toBe(true);
  });
```

并在 import 区加 `import { useUiStore } from "../stores/ui";`（该 describe 之外新增用例需放在 `describe("AppShell 启动时序")` 内或新开 describe——**放在既有 describe 内**，beforeEach 的 `h.calls.length = 0` 不影响本用例）。

同时修改 `src/__tests__/app-components.spec.ts`，在 `describe("AppList")` 内追加：

```ts
  it("应用带 homepage 时列表展示抓取到的图标", async () => {
    fetchIcon.mockResolvedValue("data:image/png;base64,AA==");
    const wrapper = mount(AppList, { props: { apps: [git], selected: null } });
    await flushPromises();
    expect(fetchIcon).toHaveBeenCalledWith("https://gitforwindows.org");
    expect(wrapper.find("img").attributes("src")).toBe("data:image/png;base64,AA==");
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/store-page.spec.ts src/__tests__/appshell.spec.ts src/__tests__/app-components.spec.ts`
Expected: FAIL —— `../pages/StorePage.vue` 不存在；AppList 未传 source。

- [ ] **Step 3: 实现组件**

创建 `src/components/store/PackageDetail.vue`：

```vue
<template>
  <div class="detail">
    <div v-if="!pkg" class="empty">选择一个包查看详情</div>
    <template v-else>
      <div class="head">
        <AppIcon :name="pkg.name" :size="64" :source="pkg.icon ?? pkg.homepage" />
        <div>
          <div class="title">
            {{ pkg.name }}
            <n-tag v-if="pkg.deprecated" size="small" type="warning">已弃用</n-tag>
          </div>
          <div class="sub">
            版本 {{ pkg.version }} · 来自 {{ pkg.bucket }} 桶<span v-if="pkg.license"> · {{ pkg.license }}</span>
          </div>
        </div>
      </div>

      <p v-if="pkg.description" class="desc">{{ pkg.description }}</p>

      <div class="actions">
        <n-button v-if="pkg.homepage" size="small" @click="openHomepage">打开主页</n-button>
      </div>

      <n-descriptions :column="1" size="small" label-placement="left" class="fields">
        <n-descriptions-item v-if="pkg.homepage" label="主页">{{ pkg.homepage }}</n-descriptions-item>
        <n-descriptions-item v-if="pkg.license" label="许可证">{{ pkg.license }}</n-descriptions-item>
      </n-descriptions>

      <div class="install-hint">安装功能将在后续版本提供</div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { NButton, NDescriptions, NDescriptionsItem, NTag, useMessage } from "naive-ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "../app/AppIcon.vue";
import type { PackageEntry } from "../../api/types";

const props = defineProps<{ pkg: PackageEntry | null }>();
const message = useMessage();

async function openHomepage() {
  if (!props.pkg?.homepage) return;
  try {
    await openUrl(props.pkg.homepage);
  } catch {
    message.error("打开失败：无法打开链接");
  }
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
.install-hint {
  margin-top: 20px;
  padding-top: 12px;
  border-top: 1px solid #efeff5;
  color: #999;
  font-size: 12px;
}
.empty {
  color: #999;
  text-align: center;
  padding-top: 60px;
}
</style>
```

创建 `src/pages/StorePage.vue`：

```vue
<template>
  <div class="page">
    <div class="main">
      <div class="toolbar">
        <n-input
          v-model:value="input"
          clearable
          placeholder="搜索应用（名称或描述）"
          class="search"
          @update:value="onInput"
        />
        <n-button-group class="view-toggle">
          <n-button size="small" :type="view === 'list' ? 'primary' : 'default'" @click="view = 'list'">
            列表
          </n-button>
          <n-button size="small" :type="view === 'grid' ? 'primary' : 'default'" @click="view = 'grid'">
            卡片
          </n-button>
        </n-button-group>
        <n-button size="small" :loading="pkgs.refreshing" @click="refresh">刷新索引</n-button>
        <span v-if="pkgs.indexCount !== null" class="count">共 {{ pkgs.indexCount }} 个包</span>
      </div>

      <n-alert v-if="pkgs.error" type="error" size="small" class="alert">{{ pkgs.error }}</n-alert>
      <div v-if="pkgs.loading" class="hint">搜索中…</div>
      <div v-else-if="pkgs.results.length === 0" class="hint">没有匹配的应用</div>
      <div v-else class="results" :class="view">
        <template v-if="view === 'list'">
          <div
            v-for="p in pkgs.results"
            :key="`${p.bucket}/${p.name}`"
            class="row"
            :class="{ active: p.name === pkgs.selectedName }"
            @click="pkgs.select(p.name)"
          >
            <AppIcon :name="p.name" :size="28" :source="p.icon ?? p.homepage" />
            <div class="info">
              <div class="name">
                {{ p.name }}
                <n-tag v-if="p.deprecated" size="tiny" type="warning">已弃用</n-tag>
              </div>
              <div class="meta">{{ p.version }}<span v-if="p.bucket"> · {{ p.bucket }}</span></div>
            </div>
          </div>
        </template>
        <template v-else>
          <div
            v-for="p in pkgs.results"
            :key="`${p.bucket}/${p.name}`"
            class="card"
            :class="{ active: p.name === pkgs.selectedName }"
            @click="pkgs.select(p.name)"
          >
            <AppIcon :name="p.name" :size="48" :source="p.icon ?? p.homepage" />
            <div class="card-name">{{ p.name }}</div>
            <div class="card-meta">{{ p.version }}</div>
          </div>
        </template>
      </div>
    </div>
    <PackageDetail :pkg="pkgs.selected" class="detail" />
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NAlert, NButton, NButtonGroup, NInput, NTag } from "naive-ui";
import AppIcon from "../components/app/AppIcon.vue";
import PackageDetail from "../components/store/PackageDetail.vue";
import { usePackagesStore } from "../stores/packages";
import { useRootsStore } from "../stores/roots";

const pkgs = usePackagesStore();
const roots = useRootsStore();
const input = ref(pkgs.query);
const view = ref<"list" | "grid">("list");
let timer: ReturnType<typeof setTimeout> | null = null;

function onInput(value: string) {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    pkgs.search(value);
  }, 300);
}

function refresh() {
  pkgs.refresh();
}

watch(
  () => roots.active?.path,
  () => {
    pkgs.search(pkgs.query);
  },
);

onMounted(() => {
  pkgs.search(pkgs.query);
});

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
});
</script>

<style scoped>
.page {
  display: flex;
  height: 100%;
}
.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-bottom: 1px solid #efeff5;
}
.search {
  flex: 1;
  min-width: 0;
}
.count {
  color: #888;
  font-size: 12px;
  white-space: nowrap;
}
.alert {
  margin: 8px 8px 0;
}
.hint {
  padding: 32px;
  color: #999;
  text-align: center;
}
.results {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 8px;
}
.results.list {
  padding: 0;
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
.results.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 8px;
}
.card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 12px 8px;
  border: 1px solid #efeff5;
  border-radius: 6px;
  cursor: pointer;
}
.card:hover {
  background: #f5f7fa;
}
.card.active {
  border-color: #2080f0;
  background: #eaf3ff;
}
.card-name {
  font-size: 13px;
  text-align: center;
  word-break: break-all;
}
.card-meta {
  font-size: 12px;
  color: #888;
}
.detail {
  width: 380px;
  flex-shrink: 0;
  border-left: 1px solid #efeff5;
}
</style>
```

修改 `src/components/layout/AppShell.vue`：

```vue
<template>
  <div class="shell">
    <SideBar />
    <div class="main">
      <TopBar />
      <div class="content">
        <InstalledPage v-if="ui.activePage === 'installed'" />
        <StorePage v-else-if="ui.activePage === 'store'" />
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
import StorePage from "../../pages/StorePage.vue";
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
  maintenance: "维护（M3：缓存清理与维护操作）",
  migrate: "环境迁移（M6：注册表扫描与接管）",
  profiles: "配置集（M5：Profile 切换）",
  backup: "备份（M5：快照与恢复）",
};
const placeholderTitle = computed(() => placeholders[ui.activePage] ?? "开发中");

onMounted(async () => {
  await useLogsStore()
    .init()
    .catch(() => {});
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

修改 `src/components/app/AppList.vue` 第 11 行：

```vue
      <AppIcon :name="app.name" :size="28" :source="app.icon ?? app.homepage" />
```

修改 `src/components/app/AppDetail.vue` 第 6 行：

```vue
        <AppIcon :name="app.name" :size="64" :source="app.icon ?? app.homepage" />
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run`
Expected: 全绿（既有 36 + 新增约 16）。

- [ ] **Step 5: 类型检查**

Run: `pnpm build`
Expected: 通过（无 vue-tsc 错误）。

- [ ] **Step 6: 提交**

```bash
git add src/components/store/PackageDetail.vue src/pages/StorePage.vue src/components/layout/AppShell.vue src/components/app/AppList.vue src/components/app/AppDetail.vue src/__tests__/store-page.spec.ts src/__tests__/appshell.spec.ts src/__tests__/app-components.spec.ts
git commit -m "feat(ui): 商店页（防抖搜索/列表卡片切换/包详情）与已装图标接入"
```

---

### Task 6: 真实窗口截图验收

**Files:**
- 无代码改动（验收任务；如发现问题回到对应任务修复）

**Interfaces:**
- Consumes: Task 1-5 全部
- Produces: 截图证据（`%TEMP%\opencode\scoopui-m2b-installed.png`、`scoopui-m2b-store.png`）+ 报告

- [ ] **Step 1: 启动 dev 并等待窗口**

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
nohup pnpm tauri dev > "/c/Users/Administrator/AppData/Local/Temp/opencode/tauri-dev-m2b.log" 2>&1 &
for i in $(seq 1 60); do
  T=$(powershell -NoProfile -Command "(Get-Process scoopui -ErrorAction SilentlyContinue | Select-Object -First 1).MainWindowTitle" 2>/dev/null)
  if [ -n "$T" ]; then echo "WINDOW_TITLE=$T"; break; fi
  sleep 5
done
```

Expected: `WINDOW_TITLE=ScoopUI`。等待 10 秒让已装页图标抓取完成（网络可达时 favicon，否则首字母）。

- [ ] **Step 2: 截图已装页（图标证据）**

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
$bmp.Save("C:\Users\Administrator\AppData\Local\Temp\opencode\scoopui-m2b-installed.png", [System.Drawing.Imaging.ImageFormat]::Png)
"SAVED $w x $h"
'
```

- [ ] **Step 3: 点击侧边栏「商店」并截图**

```bash
powershell -NoProfile -Command '
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class M {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$p = Get-Process scoopui | Select-Object -First 1
[M]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 500
$r = New-Object M+RECT
[M]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
$x = $r.Left + 90; $y = $r.Top + 100
[M]::SetCursorPos($x, $y) | Out-Null
[M]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
[M]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
"CLICKED $x $y"
'
sleep 2
# 用 Step 2 的截图脚本存为 scoopui-m2b-store-click.png 并 Read 查看；若未命中「商店」（顶栏或页面不对），
# 按截图实际位置调整 y 偏移（估计：品牌区后第 2 项，约 Top+100，每项间距约 42）后重试。
```

- [ ] **Step 4: 在搜索框输入 git 并截图**

```bash
powershell -NoProfile -Command '
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class M2 {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$p = Get-Process scoopui | Select-Object -First 1
[M2]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
$r = New-Object M2+RECT
[M2]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
# 搜索框：内容区（左 + 180）内，工具条约 y = 顶 + 32（标题栏）+ 25
$x = $r.Left + 400; $y = $r.Top + 57
[M2]::SetCursorPos($x, $y) | Out-Null
[M2]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
[M2]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait("git")
"TYPED"
'
sleep 3
# 用 Step 2 的截图脚本存为 scoopui-m2b-store.png
```

- [ ] **Step 5: 验收判定（控制者亲自查看截图）**

用 Read 工具打开两张截图，核对：
- 已装页：9 个应用仍存在；部分行有真实 favicon（img），失败项为首字母头像（两者均可接受）
- 商店页：搜索框含 `git`；列表出现 `git`/`git-lfs` 等以 git 开头的结果；每行有图标或首字母；右侧详情显示包信息（版本/桶/描述）
- 无红字报错、无空白页

- [ ] **Step 6: 关闭 dev**

```bash
taskkill.exe //F //IM scoopui.exe 2>&1 | head -2
```

- [ ] **Step 7: 写入验收报告**

报告：`C:\Users\Administrator\Desktop\idear\scoopUI\.superpowers\sdd\task-m2b-acceptance.md`，含：截图路径、窗口尺寸、界面要素清单逐项核对结果、发现的问题（如有，附修复建议）。
注：本任务不提交代码（无改动）。

---

### Task 7: M2 里程碑评审（M2a + M2b 合并）

**Files:**
- 无代码改动（评审任务）

**Interfaces:**
- Consumes: M2a 全部提交 + Task 1-6 全部产物
- Produces: 评审报告（控制者按报告决定是否需要修复波）

- [ ] **Step 1: 生成评审包**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
SKILL_DIR="/c/Users/Administrator/.cache/opencode/packages/superpowers@git+https_/github.com/jnlk-cn/superpowers-cn.git/node_modules/superpowers/skills/subagent-driven-development"
bash "$SKILL_DIR/scripts/review-package" 04dba7e HEAD
```

（BASE `04dba7e` = M1b 收尾提交，即 M2a 计划约定的初始 BASE；评审范围含 M2a 后端 + M2b 前端全部。）

- [ ] **Step 2: 分派评审子代理（读源码方式，勿用大 diff）**

评审范围：
- 后端 M2a：`src-tauri/src/domain/index.rs`、`src-tauri/src/domain/icons.rs`、`src-tauri/src/commands/search.rs`、`src-tauri/src/commands/icons.rs`、`src-tauri/src/state.rs`、`src-tauri/src/lib.rs`、`src-tauri/Cargo.toml`
- 前端 M2b：`src/api/types.ts`、`src/api/commands.ts`、`src/stores/icons.ts`、`src/stores/packages.ts`、`src/components/app/**`、`src/components/store/**`、`src/pages/StorePage.vue`、`src/components/layout/AppShell.vue`、`src/__tests__/*.spec.ts`

评审重点：契约逐字对齐（`PackageEntry` snake_case、`fetch_icon` null、中文错误）；icons store 并发/负缓存正确性与内存泄漏；packages store 竞态；StorePage 防抖与卸载清理；AppIcon 回退链；测试真实性（是否有假绿）；M2 越界（安装/卸载/更新不得提前实现）。

---

## M2b 验收清单（人工）

- [ ] `pnpm test` 全绿（M1b 基线 36 + icons 5 + packages 5 + store-page 7 + AppIcon 2 新增 + AppList 1 + AppShell 1 ≈ 57）
- [ ] `pnpm build` 无类型错误
- [ ] `pnpm tauri dev` 商店页可搜索 `git` 秒出结果，列表/卡片可切换，详情正常
- [ ] 已装页/商店页图标可见（favicon 或首字母兜底），无报错红字
- [ ] 根目录切换后商店页按当前关键词重搜（人工点选验证）

## Self-Review 记录

- 规格覆盖：设计 §5 商店（搜索框 + 结果列表/可切卡片网格 + 详情；安装按钮按 progress 延后 M3）、§4.4 图标管线（懒加载=组件挂载时按需 ensure、并发 6、磁盘缓存+负缓存由 M2a 后端负责、失败静默降级首字母）、§4.3 搜索走 M2a 契约。已装页图标来源 `icon ?? homepage` 已接到 AppList/AppDetail。
- 占位符扫描：无 TBD；所有新文件含完整代码，所有修改给出精确替换块。
- 类型一致性：`PackageEntry` 在 Task 1 定义，Task 4/5 复用；`ensure/resolve` 在 Task 2 定义，Task 3 使用；`search/refresh/select/indexCount/refreshing` 在 Task 4 定义，Task 5 使用；`PackageDetail` props `pkg` 与 packages store getter `selected` 类型一致。
- 已知取舍：`fetchIcon` 逐个串行队列而非并行批量（命令本身是 async，并发 6 已由 store 控制）；刷新索引按钮为手动触发（设计 §4.3 的自动失效重建在 M4 桶更新流程接线）。
- 风险提示：截图验收的鼠标坐标依赖窗口布局，执行者需按截图迭代调整（已知 offset 基准见 Step 3/4）。
