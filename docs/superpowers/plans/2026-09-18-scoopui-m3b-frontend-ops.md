# ScoopUI M3b（前端：应用操作 / 商店安装 / 桶管理 / 维护页）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 前端接上 M3a 的 7 个操作命令：已安装详情支持更新/卸载/锁定，商店详情支持安装（桶/版本/全局），桶管理页支持增删与刷新，维护页提供缓存清理/旧版本清理/checkup/reset；所有操作走统一任务管线（自动开日志抽屉、成功后刷新数据、失败中文提示）。

**Architecture:** `src/api/commands.ts` 增加 7 个包装；`src/stores/logs.ts` 增加 `busy` getter 与 `fail(taskId, message)`（invoke 被拒时落地失败状态，避免 running 悬挂）；新建 `src/utils/tasks.ts` 的 `runTask(label, start)`（生成 taskId → logs.start → invoke → 等 `scoop-task-end`/fail → 返回 TaskRecord）；页面组件统一用它。

**Tech Stack:** Vue 3 + TS + Naive UI + Pinia + Vitest（jsdom）。无新增依赖。

## Global Constraints

- **M3a 契约（字面一致）**：
  - `installPackage(name, bucket: string|null, version: string|null, global, taskId)`
  - `uninstallPackage(name, global, purge, taskId)`
  - `updatePackage(name: string|null, all, global, taskId)`
  - `setHold(name, hold, global, taskId)`
  - `bucketAdd(name, repo: string|null, taskId)` / `bucketRemove(name, taskId)`
  - `runMaintenance(op, taskId)`，op ∈ `cleanup_all | cleanup_global | cache_rm_all | checkup | reset_all`
  - 命令 reject 为中文 string（如串行守卫「已有任务正在运行，请等待完成或取消后再试」）
- 事件（不变）：`scoop-log`、`scoop-task-end`；`logs.start` 必须在 invoke 之前
- 前端只能经 `src/api/commands.ts` 调后端；不得直接 import `@tauri-apps/api/core`（`src/api/tauri.ts` 除外）
- 所有操作按钮在 `logs.busy` 时禁用；操作开始自动展开日志抽屉（`ui.logDrawerOpen = true`）
- 成功 → 刷新受影响数据；失败 → `message.error("…（退出码 X），详见任务日志")`
- UI 文案全中文；代码不加注释；不加依赖
- 验证门：`pnpm test` 全绿、`pnpm build`（vue-tsc）无错
- 初始 BASE 提交：`f70a659`

---

### Task 1: 命令包装 + logs.busy/fail + runTask 任务助手

**Files:**
- Modify: `src/api/commands.ts`
- Modify: `src/stores/logs.ts`
- Create: `src/utils/tasks.ts`
- Test: `src/__tests__/commands.spec.ts`、`src/__tests__/logs.spec.ts`、`src/__tests__/tasks.spec.ts`

**Interfaces:**
- Consumes: 既有 `invoke`、logs store
- Produces（后续任务引用）:
  - `installPackage/uninstallPackage/updatePackage/setHold/bucketAdd/bucketRemove/runMaintenance`（`src/api/commands.ts`）
  - `useLogsStore().busy: boolean`（任一任务运行中）、`fail(taskId: string, message: string): void`
  - `runTask(label: string, start: (taskId: string) => Promise<void>): Promise<TaskRecord>`（`src/utils/tasks.ts`）
  - `newTaskId(): string`

- [ ] **Step 1: 写失败测试（commands）**

在 `src/__tests__/commands.spec.ts` 的 import 中追加 `bucketAdd, bucketRemove, installPackage, runMaintenance, setHold, uninstallPackage, updatePackage`，并在「M2 命令名与参数符合后端契约」用例之后追加：

```ts
  it("M3 操作命令名与参数符合后端契约", async () => {
    await installPackage("git", "main", null, false, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("install_package", {
      name: "git",
      bucket: "main",
      version: null,
      global: false,
      taskId: "t-2",
    });

    await uninstallPackage("git", false, true, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("uninstall_package", {
      name: "git",
      global: false,
      purge: true,
      taskId: "t-2",
    });

    await updatePackage(null, true, false, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("update_package", {
      name: null,
      all: true,
      global: false,
      taskId: "t-2",
    });

    await setHold("git", true, false, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("set_hold", {
      name: "git",
      hold: true,
      global: false,
      taskId: "t-2",
    });

    await bucketAdd("java", null, "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("bucket_add", {
      name: "java",
      repo: null,
      taskId: "t-2",
    });

    await bucketRemove("java", "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("bucket_remove", { name: "java", taskId: "t-2" });

    await runMaintenance("checkup", "t-2");
    expect(invokeMock).toHaveBeenLastCalledWith("run_maintenance", { op: "checkup", taskId: "t-2" });
  });
```

- [ ] **Step 2: 运行确认失败**

Run: `pnpm vitest run src/__tests__/commands.spec.ts` → FAIL（导出不存在）。

- [ ] **Step 3: 实现命令包装**

`src/api/commands.ts` 末尾追加：

```ts
export const installPackage = (
  name: string,
  bucket: string | null,
  version: string | null,
  global: boolean,
  taskId: string,
) => invoke<void>("install_package", { name, bucket, version, global, taskId });
export const uninstallPackage = (name: string, global: boolean, purge: boolean, taskId: string) =>
  invoke<void>("uninstall_package", { name, global, purge, taskId });
export const updatePackage = (name: string | null, all: boolean, global: boolean, taskId: string) =>
  invoke<void>("update_package", { name, all, global, taskId });
export const setHold = (name: string, hold: boolean, global: boolean, taskId: string) =>
  invoke<void>("set_hold", { name, hold, global, taskId });
export const bucketAdd = (name: string, repo: string | null, taskId: string) =>
  invoke<void>("bucket_add", { name, repo, taskId });
export const bucketRemove = (name: string, taskId: string) =>
  invoke<void>("bucket_remove", { name, taskId });
export const runMaintenance = (op: string, taskId: string) =>
  invoke<void>("run_maintenance", { op, taskId });
```

- [ ] **Step 4: 写失败测试（logs fail/busy）**

在 `src/__tests__/logs.spec.ts` 追加（沿用既有 mock 结构）：

```ts
  it("busy 反映是否有任务运行中", () => {
    const store = useLogsStore();
    expect(store.busy).toBe(false);
    store.start("t1", "安装 git");
    expect(store.busy).toBe(true);
    store.end({ taskId: "t1", code: 0, success: true });
    expect(store.busy).toBe(false);
  });

  it("fail 落地失败状态与错误行", () => {
    const store = useLogsStore();
    store.start("t1", "安装 git");
    store.fail("t1", "已有任务正在运行，请等待完成或取消后再试");
    expect(store.running["t1"]).toBe(false);
    expect(store.history[0]).toMatchObject({ taskId: "t1", success: false, code: null });
    expect(store.lines.some((l) => l.line.includes("已有任务正在运行"))).toBe(true);
  });
```

- [ ] **Step 5: 实现 logs.busy/fail**

`src/stores/logs.ts` 的 `getters` 增加：

```ts
  getters: {
    busy(state): boolean {
      return Object.values(state.running).some(Boolean);
    },
  },
```

`actions` 中 `end` 之后增加：

```ts
    fail(taskId: string, message: string) {
      this.running[taskId] = false;
      const label = this.labels[taskId] ?? taskId;
      this.history.unshift({ taskId, label, success: false, code: null, at: Date.now() });
      this.lines.push({ taskId, stream: "stderr", line: message });
    },
```

（该 store 现为 `state + actions`，新增 `getters` 块；保持 `defineStore("logs", { state, getters, actions })` 结构。）

- [ ] **Step 6: 写失败测试（tasks）**

创建 `src/__tests__/tasks.spec.ts`：

```ts
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import { useLogsStore } from "../stores/logs";
import { newTaskId, runTask } from "../utils/tasks";

describe("runTask", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("登记任务、等待结束事件并返回记录", async () => {
    const logs = useLogsStore();
    let id = "";
    const pending = runTask("安装 git", async (taskId) => {
      id = taskId;
    });
    await flushPromises();
    expect(id).toMatch(/^t-/);
    expect(logs.running[id]).toBe(true);
    expect(logs.labels[id]).toBe("安装 git");
    logs.end({ taskId: id, code: 0, success: true });
    const record = await pending;
    expect(record.success).toBe(true);
    expect(record.taskId).toBe(id);
    expect(logs.busy).toBe(false);
  });

  it("启动被拒时记录失败并返回失败记录", async () => {
    const logs = useLogsStore();
    const record = await runTask("安装 git", async () => {
      throw "已有任务正在运行，请等待完成或取消后再试";
    });
    expect(record.success).toBe(false);
    expect(record.code).toBeNull();
    expect(logs.running[record.taskId]).toBe(false);
    expect(logs.history[0].success).toBe(false);
    expect(logs.lines.some((l) => l.line.includes("已有任务正在运行"))).toBe(true);
  });

  it("newTaskId 递增且带 t- 前缀", () => {
    expect(newTaskId()).toMatch(/^t-/);
    expect(newTaskId()).not.toBe(newTaskId());
  });
});
```

Run: `pnpm vitest run src/__tests__/tasks.spec.ts` → FAIL（模块不存在）。

- [ ] **Step 7: 实现 tasks.ts**

创建 `src/utils/tasks.ts`：

```ts
import { watch } from "vue";
import { useLogsStore } from "../stores/logs";
import type { TaskRecord } from "../stores/logs";

let counter = 0;

export function newTaskId(): string {
  counter += 1;
  return `t-${Date.now()}-${counter}`;
}

export async function runTask(
  label: string,
  start: (taskId: string) => Promise<void>,
): Promise<TaskRecord> {
  const logs = useLogsStore();
  const taskId = newTaskId();
  logs.start(taskId, label);
  const done = new Promise<TaskRecord>((resolve) => {
    const stop = watch(
      () => logs.running[taskId],
      (running) => {
        if (running === false) {
          stop();
          resolve(
            logs.history.find((r) => r.taskId === taskId) ?? {
              taskId,
              label,
              success: false,
              code: null,
              at: Date.now(),
            },
          );
        }
      },
    );
  });
  try {
    await start(taskId);
  } catch (e) {
    logs.fail(taskId, String(e));
  }
  return done;
}
```

- [ ] **Step 8: 运行验证**

Run: `pnpm vitest run src/__tests__/commands.spec.ts src/__tests__/logs.spec.ts src/__tests__/tasks.spec.ts` → 全绿；`pnpm test` 全量绿；`pnpm build` 通过。

- [ ] **Step 9: 提交**

```bash
git add src/api/commands.ts src/stores/logs.ts src/utils/tasks.ts src/__tests__/commands.spec.ts src/__tests__/logs.spec.ts src/__tests__/tasks.spec.ts
git commit -m "feat(ui): M3 操作命令包装与 runTask 任务助手"
```

---

### Task 2: AppDetail 更新 / 卸载 / 锁定

**Files:**
- Modify: `src/components/app/AppDetail.vue`
- Test: `src/__tests__/app-ops.spec.ts`（新建）、`src/__tests__/app-components.spec.ts`（补 mock 键）

**Interfaces:**
- Consumes: Task 1 的 `runTask`、`logs.busy`、`updatePackage/uninstallPackage/setHold`
- Produces: AppDetail 操作区按钮（更新 / 卸载确认 / 锁定|解除锁定），成功后 `apps.load()`，失败 `message.error`

- [ ] **Step 1: 写失败测试**

创建 `src/__tests__/app-ops.spec.ts`：

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { NPopconfirm } from "naive-ui";

const messageError = vi.hoisted(() => vi.fn());
const messageSuccess = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  listInstalledApps: vi.fn(),
  updatePackage: vi.fn(),
  uninstallPackage: vi.fn(),
  setHold: vi.fn(),
  fetchIcon: vi.fn().mockResolvedValue(null),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: messageSuccess,
      info: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(() => Promise.resolve()),
  openUrl: vi.fn(() => Promise.resolve()),
}));

import AppDetail from "../components/app/AppDetail.vue";
import { useAppsStore } from "../stores/apps";
import { useLogsStore } from "../stores/logs";
import type { InstalledApp } from "../api/types";

const git: InstalledApp = {
  name: "git",
  version: "2.55.0.5",
  bucket: "main",
  architecture: "64bit",
  hold: false,
  updated: null,
  description: "git",
  homepage: null,
  icon: null,
  depends: [],
  persist: [],
  bin: ["git"],
  app_dir: "C:\\scoop\\apps\\git",
};

function findButton(wrapper: ReturnType<typeof mount>, text: string) {
  const button = wrapper.findAll("button").find((b) => b.text().includes(text));
  expect(button).toBeTruthy();
  return button!;
}

describe("AppDetail 操作", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listInstalledApps.mockResolvedValue([git]);
  });

  it("更新成功后刷新已安装列表", async () => {
    mocks.updatePackage.mockResolvedValue(undefined);
    const wrapper = mount(AppDetail, { props: { app: git } });
    await findButton(wrapper, "更新").trigger("click");
    await flushPromises();
    expect(mocks.updatePackage).toHaveBeenCalledWith("git", false, false, expect.any(String));
    const taskId = mocks.updatePackage.mock.calls[0][3];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.listInstalledApps).toHaveBeenCalledTimes(1);
  });

  it("卸载失败提示错误且不刷新", async () => {
    mocks.uninstallPackage.mockResolvedValue(undefined);
    const wrapper = mount(AppDetail, { props: { app: git } });
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.uninstallPackage).toHaveBeenCalledWith("git", false, false, expect.any(String));
    const taskId = mocks.uninstallPackage.mock.calls[0][3];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("操作失败（退出码 1），详见任务日志");
    expect(mocks.listInstalledApps).not.toHaveBeenCalled();
  });

  it("锁定调用 setHold(hold=true)", async () => {
    mocks.setHold.mockResolvedValue(undefined);
    const wrapper = mount(AppDetail, { props: { app: git } });
    await findButton(wrapper, "锁定").trigger("click");
    await flushPromises();
    expect(mocks.setHold).toHaveBeenCalledWith("git", true, false, expect.any(String));
  });

  it("有任务运行中时操作按钮禁用", () => {
    useLogsStore().start("t1", "更新 git");
    const wrapper = mount(AppDetail, { props: { app: git } });
    expect(findButton(wrapper, "更新").attributes("disabled")).toBeDefined();
    expect(findButton(wrapper, "卸载").attributes("disabled")).toBeDefined();
    expect(findButton(wrapper, "锁定").attributes("disabled")).toBeDefined();
  });
});
```

同时 `src/__tests__/app-components.spec.ts` 的 `otherCommands` 增加 `updatePackage: vi.fn()`、`uninstallPackage: vi.fn()`、`setHold: vi.fn()`（AppDetail 模块导入需要键存在）。

Run: `pnpm vitest run src/__tests__/app-ops.spec.ts` → FAIL（按钮不存在）。

- [ ] **Step 2: 实现 AppDetail 操作区**

`src/components/app/AppDetail.vue` 的 `.actions` 块替换为：

```vue
      <div class="actions">
        <n-button v-if="app.homepage" size="small" @click="openHomepage">打开主页</n-button>
        <n-button size="small" @click="openDir">打开安装目录</n-button>
        <n-button size="small" :disabled="busy" @click="doUpdate">更新</n-button>
        <n-popconfirm @positive-click="doUninstall">
          <template #trigger>
            <n-button size="small" type="error" :disabled="busy">卸载</n-button>
          </template>
          确定卸载 {{ app.name }}？程序文件将被删除（持久化数据保留）。
        </n-popconfirm>
        <n-button size="small" :disabled="busy" @click="toggleHold">
          {{ app.hold ? "解除锁定" : "锁定" }}
        </n-button>
      </div>
```

script 部分改为：

```ts
import { computed } from "vue";
import { NButton, NDescriptions, NDescriptionsItem, NPopconfirm, NTag, useMessage } from "naive-ui";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "./AppIcon.vue";
import type { InstalledApp } from "../../api/types";
import { setHold, uninstallPackage, updatePackage } from "../../api/commands";
import { useAppsStore } from "../../stores/apps";
import { useLogsStore } from "../../stores/logs";
import { useUiStore } from "../../stores/ui";
import { runTask } from "../../utils/tasks";
import type { TaskRecord } from "../../stores/logs";

const props = defineProps<{ app: InstalledApp | null }>();
const message = useMessage();
const apps = useAppsStore();
const logs = useLogsStore();
const ui = useUiStore();
const busy = computed(() => logs.busy);

async function afterTask(record: TaskRecord) {
  if (record.success) {
    await apps.load();
  } else {
    message.error(`操作失败（退出码 ${record.code ?? "未知"}），详见任务日志`);
  }
}

async function doUpdate() {
  if (!props.app) return;
  const name = props.app.name;
  ui.logDrawerOpen = true;
  const record = await runTask(`更新 ${name}`, (taskId) => updatePackage(name, false, false, taskId));
  await afterTask(record);
}

async function doUninstall() {
  if (!props.app) return;
  const name = props.app.name;
  ui.logDrawerOpen = true;
  const record = await runTask(`卸载 ${name}`, (taskId) => uninstallPackage(name, false, false, taskId));
  await afterTask(record);
}

async function toggleHold() {
  if (!props.app) return;
  const name = props.app.name;
  const next = !props.app.hold;
  ui.logDrawerOpen = true;
  const record = await runTask(`${next ? "锁定" : "解除锁定"} ${name}`, (taskId) =>
    setHold(name, next, false, taskId),
  );
  await afterTask(record);
}

async function openHomepage() { /* 保持原实现 */ }
async function openDir() { /* 保持原实现 */ }
```

（`openHomepage`/`openDir` 原实现逐字保留。）

- [ ] **Step 3: 运行验证**

Run: `pnpm vitest run src/__tests__/app-ops.spec.ts src/__tests__/app-components.spec.ts` → 全绿；`pnpm test` 全量绿（预计 73+）；`pnpm build` 通过。

- [ ] **Step 4: 提交**

```bash
git add src/components/app/AppDetail.vue src/__tests__/app-ops.spec.ts src/__tests__/app-components.spec.ts
git commit -m "feat(ui): 已安装详情支持更新/卸载/锁定"
```

---

### Task 3: 商店详情安装（桶 / 版本 / 全局）

**Files:**
- Modify: `src/components/store/PackageDetail.vue`
- Test: `src/__tests__/store-page.spec.ts`

**Interfaces:**
- Consumes: Task 1 的 `runTask`、`installPackage`、`logs.busy`；`useAppsStore`
- Produces: 安装面板（版本输入 + 全局复选 + 安装按钮），安装目标 `pkg.bucket/pkg.name[@version]`，成功后 `message.success` + `apps.load()`

- [ ] **Step 1: 写失败测试**

`src/__tests__/store-page.spec.ts`：mocks 对象增加 `installPackage: vi.fn().mockResolvedValue(undefined)`、`listInstalledApps` 已有；追加用例：

```ts
  it("安装按钮调用 installPackage 并在成功后刷新已安装列表", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git", { description: "git" })]);
    const wrapper = mount(StorePage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("安装"));
    expect(button).toBeTruthy();
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("git", "main", null, false, expect.any(String));
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.listInstalledApps).toHaveBeenCalled();
  });

  it("版本与全局选项传入安装命令", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git")]);
    const wrapper = mount(StorePage);
    await flushPromises();
    await wrapper.find('input[placeholder="版本（留空 = 最新）"]').setValue("2.7.0");
    await wrapper.find('input[type="checkbox"]').setValue(true);
    const button = wrapper.findAll("button").find((b) => b.text().includes("安装"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.installPackage).toHaveBeenCalledWith("git", "main", "2.7.0", true, expect.any(String));
  });

  it("安装失败提示错误", async () => {
    mocks.searchPackages.mockResolvedValue([pkg("git")]);
    const wrapper = mount(StorePage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("安装"));
    await button!.trigger("click");
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("安装失败（退出码 1），详见任务日志");
  });
```

并在 import 区补 `import { useLogsStore } from "../stores/logs";`。注意既有用例中「安装功能将在后续版本提供」的断言不存在，无需改动其他用例；但 `.detail` 文本断言（描述）仍成立。

Run: `pnpm vitest run src/__tests__/store-page.spec.ts` → FAIL（无安装按钮）。

- [ ] **Step 2: 实现安装面板**

`src/components/store/PackageDetail.vue`：

模板把 `<div class="install-hint">安装功能将在后续版本提供</div>` 替换为：

```vue
      <div class="install">
        <div class="install-row">
          <n-input v-model:value="version" size="small" placeholder="版本（留空 = 最新）" class="version" />
          <n-checkbox v-model:checked="globalInstall">全局安装</n-checkbox>
        </div>
        <div class="install-row">
          <n-button type="primary" size="small" :disabled="busy" @click="doInstall">安装</n-button>
          <span class="install-source">
            来自 {{ pkg.bucket }} 桶<span v-if="globalInstall">；全局安装可能需要管理员权限</span>
          </span>
        </div>
      </div>
```

script 改为：

```ts
import { computed, ref } from "vue";
import {
  NButton,
  NCheckbox,
  NDescriptions,
  NDescriptionsItem,
  NInput,
  NTag,
  useMessage,
} from "naive-ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import AppIcon from "../app/AppIcon.vue";
import type { PackageEntry } from "../../api/types";
import { installPackage } from "../../api/commands";
import { useAppsStore } from "../../stores/apps";
import { useLogsStore } from "../../stores/logs";
import { useUiStore } from "../../stores/ui";
import { runTask } from "../../utils/tasks";

const props = defineProps<{ pkg: PackageEntry | null }>();
const message = useMessage();
const apps = useAppsStore();
const logs = useLogsStore();
const ui = useUiStore();
const version = ref("");
const globalInstall = ref(false);
const busy = computed(() => logs.busy);

async function doInstall() {
  if (!props.pkg) return;
  const pkg = props.pkg;
  const ver = version.value.trim() === "" ? null : version.value.trim();
  ui.logDrawerOpen = true;
  const record = await runTask(`安装 ${pkg.name}`, (taskId) =>
    installPackage(pkg.name, pkg.bucket, ver, globalInstall.value, taskId),
  );
  if (record.success) {
    message.success(`已安装 ${pkg.name}`);
    await apps.load();
  } else {
    message.error(`安装失败（退出码 ${record.code ?? "未知"}），详见任务日志`);
  }
}

async function openHomepage() { /* 保持原实现 */ }
```

样式把 `.install-hint` 替换为：

```css
.install {
  margin-top: 20px;
  padding-top: 12px;
  border-top: 1px solid #efeff5;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.install-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.version {
  width: 180px;
}
.install-source {
  color: #999;
  font-size: 12px;
}
```

- [ ] **Step 3: 运行验证**

Run: `pnpm vitest run src/__tests__/store-page.spec.ts` → 全绿；`pnpm test` 全量绿；`pnpm build` 通过。

- [ ] **Step 4: 提交**

```bash
git add src/components/store/PackageDetail.vue src/__tests__/store-page.spec.ts
git commit -m "feat(ui): 商店详情支持安装（版本/全局）"
```

---

### Task 4: 桶管理页（添加 / 删除 / 更新）

**Files:**
- Modify: `src/pages/BucketsPage.vue`
- Test: `src/__tests__/buckets-page.spec.ts`（新建）

**Interfaces:**
- Consumes: Task 1 的 `runTask`、`bucketAdd/bucketRemove/updatePackage`、`logs.busy`；`buckets.load()`、`roots.load()`
- Produces: 添加桶（名称+可选仓库）、更新 Scoop 与桶、每行删除确认；成功后 `buckets.load()` + `roots.load()`

- [ ] **Step 1: 写失败测试**

创建 `src/__tests__/buckets-page.spec.ts`：

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { NPopconfirm } from "naive-ui";

const messageError = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  listBuckets: vi.fn(),
  detectRoots: vi.fn().mockResolvedValue([]),
  getActiveRoot: vi.fn().mockResolvedValue(null),
  bucketAdd: vi.fn().mockResolvedValue(undefined),
  bucketRemove: vi.fn().mockResolvedValue(undefined),
  updatePackage: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

import BucketsPage from "../pages/BucketsPage.vue";
import { useBucketsStore } from "../stores/buckets";
import { useLogsStore } from "../stores/logs";

describe("BucketsPage 操作", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.listBuckets.mockResolvedValue([
      { name: "main", source: "https://github.com/ScoopInstaller/Main", is_git: true, manifest_count: 1641 },
    ]);
  });

  it("添加桶调用 bucketAdd 并刷新列表", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    await wrapper.find('input[placeholder="桶名称（如 extras）"]').setValue("java");
    await wrapper.find('input[placeholder="仓库地址（可选，已知桶可留空）"]').setValue("https://example.com/java.git");
    const button = wrapper.findAll("button").find((b) => b.text().includes("添加桶"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.bucketAdd).toHaveBeenCalledWith("java", "https://example.com/java.git", expect.any(String));
    const taskId = mocks.bucketAdd.mock.calls[0][2];
    useLogsStore().end({ taskId, code: 0, success: true });
    await flushPromises();
    expect(mocks.listBuckets).toHaveBeenCalledTimes(2);
  });

  it("已知桶仓库留空时传 null", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    await wrapper.find('input[placeholder="桶名称（如 extras）"]').setValue("extras");
    const button = wrapper.findAll("button").find((b) => b.text().includes("添加桶"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.bucketAdd).toHaveBeenCalledWith("extras", null, expect.any(String));
  });

  it("删除桶需确认且失败提示错误", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.bucketRemove).toHaveBeenCalledWith("main", expect.any(String));
    const taskId = mocks.bucketRemove.mock.calls[0][1];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("操作失败（退出码 1），详见任务日志");
    expect(mocks.listBuckets).toHaveBeenCalledTimes(1);
  });

  it("更新 Scoop 与桶调用 updatePackage(null,false,false)", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("更新 Scoop 与桶"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.updatePackage).toHaveBeenCalledWith(null, false, false, expect.any(String));
  });

  it("有任务运行时按钮禁用", async () => {
    useLogsStore().start("t1", "测试任务");
    const wrapper = mount(BucketsPage);
    await flushPromises();
    const addButton = wrapper.findAll("button").find((b) => b.text().includes("添加桶"));
    expect(addButton!.attributes("disabled")).toBeDefined();
  });
});
```

Run: `pnpm vitest run src/__tests__/buckets-page.spec.ts` → FAIL。

- [ ] **Step 2: 实现 BucketsPage**

`src/pages/BucketsPage.vue` 全文替换为：

```vue
<template>
  <div class="page">
    <div class="toolbar">
      <n-input v-model:value="newName" size="small" placeholder="桶名称（如 extras）" class="name" />
      <n-input v-model:value="newRepo" size="small" placeholder="仓库地址（可选，已知桶可留空）" class="repo" />
      <n-button size="small" :disabled="busy || !canAdd" @click="doAdd">添加桶</n-button>
      <n-button size="small" :disabled="busy" @click="doUpdateAll">更新 Scoop 与桶</n-button>
    </div>
    <n-alert v-if="store.error" type="error" size="small" class="alert">{{ store.error }}</n-alert>
    <n-table :bordered="false" size="small">
      <thead>
        <tr>
          <th>名称</th>
          <th>来源</th>
          <th>清单数</th>
          <th>类型</th>
          <th>操作</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="b in store.buckets" :key="b.name">
          <td>{{ b.name }}</td>
          <td class="source">{{ b.source ?? "（本地目录，非 git）" }}</td>
          <td>{{ b.manifest_count }}</td>
          <td>{{ b.is_git ? "git" : "目录" }}</td>
          <td>
            <n-popconfirm @positive-click="doRemove(b.name)">
              <template #trigger>
                <n-button size="tiny" type="error" :disabled="busy">删除</n-button>
              </template>
              确定删除桶 {{ b.name }}？该桶提供的应用将无法安装/更新。
            </n-popconfirm>
          </td>
        </tr>
      </tbody>
    </n-table>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { NAlert, NButton, NInput, NPopconfirm, NTable, useMessage } from "naive-ui";
import { bucketAdd, bucketRemove, updatePackage } from "../api/commands";
import { useBucketsStore } from "../stores/buckets";
import { useLogsStore } from "../stores/logs";
import { useRootsStore } from "../stores/roots";
import { useUiStore } from "../stores/ui";
import { runTask } from "../utils/tasks";
import type { TaskRecord } from "../stores/logs";

const store = useBucketsStore();
const roots = useRootsStore();
const logs = useLogsStore();
const ui = useUiStore();
const message = useMessage();
const newName = ref("");
const newRepo = ref("");
const busy = computed(() => logs.busy);
const canAdd = computed(() => newName.value.trim().length > 0);

async function reload() {
  await store.load();
  await roots.load();
}

async function afterTask(record: TaskRecord) {
  if (record.success) {
    await reload();
  } else {
    message.error(`操作失败（退出码 ${record.code ?? "未知"}），详见任务日志`);
  }
}

async function doAdd() {
  const name = newName.value.trim();
  if (!name) return;
  const repo = newRepo.value.trim() === "" ? null : newRepo.value.trim();
  ui.logDrawerOpen = true;
  const record = await runTask(`添加桶 ${name}`, (taskId) => bucketAdd(name, repo, taskId));
  if (record.success) {
    newName.value = "";
    newRepo.value = "";
  }
  await afterTask(record);
}

async function doRemove(name: string) {
  ui.logDrawerOpen = true;
  const record = await runTask(`删除桶 ${name}`, (taskId) => bucketRemove(name, taskId));
  await afterTask(record);
}

async function doUpdateAll() {
  ui.logDrawerOpen = true;
  const record = await runTask("更新 Scoop 与桶", (taskId) => updatePackage(null, false, false, taskId));
  await afterTask(record);
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
  gap: 8px;
  margin-bottom: 12px;
}
.name {
  width: 200px;
}
.repo {
  flex: 1;
  min-width: 0;
}
.alert {
  margin-bottom: 12px;
}
.source {
  font-family: Consolas, monospace;
  font-size: 12px;
  word-break: break-all;
}
</style>
```

- [ ] **Step 3: 运行验证**

Run: `pnpm vitest run src/__tests__/buckets-page.spec.ts` → 全绿；`pnpm test` 全量绿；`pnpm build` 通过。

- [ ] **Step 4: 提交**

```bash
git add src/pages/BucketsPage.vue src/__tests__/buckets-page.spec.ts
git commit -m "feat(ui): 桶管理页支持添加/删除/更新"
```

---

### Task 5: 维护页 + AppShell 集成

**Files:**
- Create: `src/pages/MaintenancePage.vue`
- Modify: `src/components/layout/AppShell.vue`
- Test: `src/__tests__/maintenance-page.spec.ts`（新建）、`src/__tests__/appshell.spec.ts`

**Interfaces:**
- Consumes: Task 1 的 `runTask`、`runMaintenance`、`logs.busy`
- Produces: `MaintenancePage.vue`（5 个操作：cleanup_all / cleanup_global / cache_rm_all / checkup / reset_all）；AppShell maintenance 分支

- [ ] **Step 1: 写失败测试**

创建 `src/__tests__/maintenance-page.spec.ts`：

```ts
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { NPopconfirm } from "naive-ui";

const messageError = vi.hoisted(() => vi.fn());
const mocks = vi.hoisted(() => ({
  runMaintenance: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../api/commands", () => mocks);

vi.mock("naive-ui", async () => {
  const actual = await vi.importActual<typeof import("naive-ui")>("naive-ui");
  return {
    ...actual,
    useMessage: () => ({
      error: messageError,
      success: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
    }),
  };
});

import MaintenancePage from "../pages/MaintenancePage.vue";
import { useLogsStore } from "../stores/logs";

function clickButton(wrapper: ReturnType<typeof mount>, text: string) {
  const button = wrapper.findAll("button").find((b) => b.text().includes(text));
  expect(button).toBeTruthy();
  return button!;
}

describe("MaintenancePage", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mocks.runMaintenance.mockResolvedValue(undefined);
  });

  it("直接操作按钮调用对应 op", async () => {
    const wrapper = mount(MaintenancePage);
    await clickButton(wrapper, "运行检查").trigger("click");
    await flushPromises();
    expect(mocks.runMaintenance).toHaveBeenCalledWith("checkup", expect.any(String));
  });

  it("需确认的操作在确认后调用", async () => {
    const wrapper = mount(MaintenancePage);
    const confirms = wrapper.findAllComponents(NPopconfirm);
    expect(confirms.length).toBeGreaterThanOrEqual(4);
    await confirms[0].vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.runMaintenance).toHaveBeenCalledWith("cache_rm_all", expect.any(String));
  });

  it("失败提示错误", async () => {
    const wrapper = mount(MaintenancePage);
    await clickButton(wrapper, "运行检查").trigger("click");
    await flushPromises();
    const taskId = mocks.runMaintenance.mock.calls[0][1];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalledWith("操作失败（退出码 1），详见任务日志");
  });

  it("任务运行中禁用按钮", () => {
    useLogsStore().start("t1", "维护");
    const wrapper = mount(MaintenancePage);
    expect(clickButton(wrapper, "运行检查").attributes("disabled")).toBeDefined();
  });
});
```

`src/__tests__/appshell.spec.ts`：`stubs` 增加 `MaintenancePage: true`，并追加用例：

```ts
  it("维护页渲染 MaintenancePage 而非占位", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    useUiStore().setPage("maintenance");
    const wrapper = mount(AppShell, { global: { plugins: [pinia], stubs } });
    expect(wrapper.find("maintenance-page-stub").exists()).toBe(true);
  });
```

Run: `pnpm vitest run src/__tests__/maintenance-page.spec.ts` → FAIL。

- [ ] **Step 2: 实现 MaintenancePage**

创建 `src/pages/MaintenancePage.vue`：

```vue
<template>
  <div class="page">
    <h3>维护操作</h3>
    <p class="tip">所有操作会真实执行，进度见底部任务日志抽屉。</p>
    <div class="grid">
      <div v-for="op in ops" :key="op.key" class="card">
        <div class="title">{{ op.title }}</div>
        <div class="desc">{{ op.desc }}</div>
        <n-popconfirm v-if="op.confirm" @positive-click="run(op)">
          <template #trigger>
            <n-button size="small" :disabled="busy">{{ op.button }}</n-button>
          </template>
          {{ op.confirm }}
        </n-popconfirm>
        <n-button v-else size="small" :disabled="busy" @click="run(op)">{{ op.button }}</n-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { NButton, NPopconfirm, useMessage } from "naive-ui";
import { runMaintenance } from "../api/commands";
import { useLogsStore } from "../stores/logs";
import { useUiStore } from "../stores/ui";
import { runTask } from "../utils/tasks";

interface MaintenanceOp {
  key: string;
  title: string;
  desc: string;
  button: string;
  confirm?: string;
}

const ops: MaintenanceOp[] = [
  {
    key: "cache_rm_all",
    title: "清理下载缓存",
    desc: "删除所有已下载的安装包缓存，下次安装需重新下载",
    button: "清理缓存",
    confirm: "确定清空下载缓存？",
  },
  {
    key: "cleanup_all",
    title: "清理旧版本（用户应用）",
    desc: "删除用户应用中保留的历史版本目录",
    button: "清理旧版本",
    confirm: "确定清理用户应用的旧版本？",
  },
  {
    key: "cleanup_global",
    title: "清理旧版本（全局应用）",
    desc: "删除全局应用中保留的历史版本目录",
    button: "清理全局旧版本",
    confirm: "确定清理全局应用的旧版本？",
  },
  {
    key: "checkup",
    title: "检查问题",
    desc: "运行 scoop checkup 诊断环境与配置问题",
    button: "运行检查",
  },
  {
    key: "reset_all",
    title: "重置 shims",
    desc: "为全部应用重建 shim（用于解决命令冲突）",
    button: "重置全部 shims",
    confirm: "确定重置全部应用的 shim？",
  },
];

const logs = useLogsStore();
const ui = useUiStore();
const message = useMessage();
const busy = computed(() => logs.busy);

async function run(op: MaintenanceOp) {
  ui.logDrawerOpen = true;
  const record = await runTask(op.title, (taskId) => runMaintenance(op.key, taskId));
  if (!record.success) {
    message.error(`操作失败（退出码 ${record.code ?? "未知"}），详见任务日志`);
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
  margin: 0 0 8px;
  font-size: 14px;
}
.tip {
  color: #999;
  font-size: 12px;
  margin: 0 0 14px;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
}
.card {
  border: 1px solid #efeff5;
  border-radius: 6px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: flex-start;
}
.title {
  font-weight: 600;
  font-size: 13px;
}
.desc {
  color: #888;
  font-size: 12px;
  flex: 1;
}
</style>
```

- [ ] **Step 3: AppShell 集成**

`src/components/layout/AppShell.vue`：

- 模板在 `BucketsPage` 分支后加：

```vue
        <MaintenancePage v-else-if="ui.activePage === 'maintenance'" />
```

- script import `MaintenancePage from "../../pages/MaintenancePage.vue";`
- `placeholders` 中删除 `maintenance` 行（保留 migrate/profiles/backup）

- [ ] **Step 4: 运行验证**

Run: `pnpm vitest run src/__tests__/maintenance-page.spec.ts src/__tests__/appshell.spec.ts` → 全绿；`pnpm test` 全量绿；`pnpm build` 通过。

- [ ] **Step 5: 提交**

```bash
git add src/pages/MaintenancePage.vue src/components/layout/AppShell.vue src/__tests__/maintenance-page.spec.ts src/__tests__/appshell.spec.ts
git commit -m "feat(ui): 维护页（缓存/旧版本/checkup/reset）与 AppShell 集成"
```

---

### Task 6: 真实窗口验收（安装/卸载一个小应用）

**Files:**
- 无代码改动（验收任务；发现问题回到对应任务修复）

**Interfaces:**
- Consumes: Task 1-5 全部 + M3a 后端
- Produces: 截图证据（`%TEMP%\opencode\scoopui-m3b-*.png`）+ 报告

- [ ] **Step 1: 启动 dev 并等待窗口**

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
nohup pnpm tauri dev > "/c/Users/Administrator/AppData/Local/Temp/opencode/tauri-dev-m3b.log" 2>&1 &
for i in $(seq 1 72); do
  T=$(powershell -NoProfile -Command "(Get-Process scoopui -ErrorAction SilentlyContinue | Select-Object -First 1).MainWindowTitle" 2>/dev/null)
  if [ -n "$T" ]; then echo "WINDOW_TITLE=$T"; break; fi
  sleep 5
done
sleep 6
```

验收应用选 **`jq`**（真实存在于 main 桶、当前未安装、体积小）。先确认未安装：

```bash
ls "C:/Users/Administrator/scoop/apps" | grep -x "jq" || echo "jq 未安装"
```

- [ ] **Step 2: 商店安装 jq**

复用 M2b 的点击/粘贴脚本（`C:\Users\Administrator\AppData\Local\Temp\opencode\scoop-click.ps1`、`scoop-shot.ps1`；如不存在则重建 M2b 计划 Task 6 Step 2/3 的脚本）：
1. 侧边栏「商店」（rel 50,164）
2. 搜索框（rel 410,101）粘贴 `jq`（`Set-Clipboard -Value 'jq'` + `^a^v`）
3. 右侧详情面板点击「安装」按钮（按截图定位，约 detail 区中部）
4. 等待任务完成（约 20-40s），截图 `scoopui-m3b-install.png`，核对：日志抽屉出现下载/安装输出、应用出现在「已安装」列表

- [ ] **Step 3: 锁定/解锁 + 卸载**

1. 已安装页选中 `jq`，点「锁定」→ 截图核对锁定徽标出现
2. 点「解除锁定」→ 徽标消失
3. 点「卸载」→ 确认弹窗 → 确认 → 等待完成 → 列表中 `jq` 消失
4. 截图 `scoopui-m3b-uninstall.png`

- [ ] **Step 4: 维护页与桶管理页冒烟**

1. 维护页：点「运行检查」(checkup) → 日志有输出；截图
2. 桶管理页：确认 4 个桶与清单数；截图（不真实增删桶，避免改用户环境）

- [ ] **Step 5: 验收判定（控制者亲自核对截图）**

- 安装：`jq` 出现在已安装列表；`C:\Users\Administrator\scoop\apps\jq` 存在
- 卸载：`jq` 从列表消失；目录不存在
- 锁定/解锁：hold 徽标变化
- 维护/桶页：无红字错误、按钮与日志工作
- 串行：任一任务运行中其他操作按钮为禁用

- [ ] **Step 6: 关闭 dev**

```bash
taskkill.exe //F //IM scoopui.exe 2>&1 | head -2
```

- [ ] **Step 7: 写验收报告**

`.superpowers/sdd/task-m3b-acceptance.md`：截图路径、每步结果、问题与修复建议。本任务不提交代码。

---

### Task 7: M3 里程碑评审（M3a + M3b）

**Files:**
- 无代码改动（评审任务）

**Interfaces:**
- Consumes: M3a 全部 + Task 1-6 产物
- Produces: 评审报告（控制者决定是否修复波）

- [ ] **Step 1: 生成评审包**

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
SKILL_DIR="/c/Users/Administrator/.cache/opencode/packages/superpowers@git+https_/github.com/jnlk-cn/superpowers-cn.git/node_modules/superpowers/skills/subagent-driven-development"
bash "$SKILL_DIR/scripts/review-package" cde335c HEAD
```

- [ ] **Step 2: 分派评审子代理（读源码方式，勿用大 diff）**

后端范围：`src-tauri/src/domain/scoopops.rs`、`src-tauri/src/commands/runner.rs`、`src-tauri/src/commands/ops.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`。
前端范围：`src/api/commands.ts`、`src/stores/logs.ts`、`src/utils/tasks.ts`、`src/components/app/AppDetail.vue`、`src/components/store/PackageDetail.vue`、`src/pages/BucketsPage.vue`、`src/pages/MaintenancePage.vue`、`src/components/layout/AppShell.vue`、`src/__tests__/*.spec.ts`。
评审重点：命令/参数契约；串行守卫与 running 悬挂；runTask 的事件等待与失败路径；成功后刷新链路；危险操作确认；任务运行中禁用；测试真实性；M4+ 越界。

---

## M3b 验收清单（人工）

- [ ] `pnpm test` 全绿（预计 73+）
- [ ] `pnpm build` 无类型错误
- [ ] 商店安装 `jq` 成功并在已安装列表可见（真实环境）
- [ ] 已安装详情：更新/锁定/解锁/卸载可用，卸载真实移除
- [ ] 桶管理页展示 4 桶，添加/删除/更新按钮与确认正常（真实增删人工决定）
- [ ] 维护页 5 个操作可运行，日志抽屉实时输出
- [ ] 任务运行中所有操作按钮禁用；重复操作不会并发

## Self-Review 记录

- 规格覆盖：设计 §5 已安装（操作按钮）、商店（安装 选桶/版本/全局）、桶管理、维护；§4.2 命令执行（流式/取消复用）；M3 里程碑表全部对应到 Task 1-5，Task 6 真实验收，Task 7 评审。
- 占位符扫描：无 TBD；所有新文件含完整代码，修改点给出精确替换块。
- 类型一致性：命令名/参数与 M3a Global Constraints 逐字一致；`TaskRecord` 来自 logs store；`runTask` 签名在 Task 1 定义、Task 2-5 使用；`MaintenanceOp` 的 key 与后端 `maintenance_args` 白名单一致（cleanup_all/cleanup_global/cache_rm_all/checkup/reset_all）。
- 已知取舍：`purge`（删除持久化数据）命令已具备但 UI 暂不暴露（YAGNI，避免误删）；安装桶选择直接用搜索结果所在桶（同名跨桶时用户选择具体结果即可），未做额外下拉；`update -a`（更新全部应用）无入口，留待 M4 更新检测。
- 风险：`runTask` 依赖事件到达；若后端 spawn 成功但事件丢失，任务会悬挂——现有事件链在 M1a 已验证，Task 6 真实验收兜底。
