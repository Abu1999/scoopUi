# ScoopUI M4 评审修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 M4 里程碑评审确认的推荐集问题：①后端 `set_bucket_source` 桶名路径穿越；②`configfile::set_key` 对「合法但非对象」JSON 静默重置且不备份；③前端出口未知（`in_china=null`）误报「可直连」；④`apps.load()` 陈旧响应覆盖；⑤`SettingsPage.loadMirror()` 失败静默；⑥补错误分支测试与 mock 契约。

**Architecture:** 纯修复，不改契约/命令/返回结构。后端在 `commands/mirrors.rs` 加桶名白名单校验、`domain/configfile.rs` 重写 `set_key` 的备份判定；前端改 `AccelerationSection.vue`（三分支提示）、`stores/apps.ts`（seq 防护，复用 `stores/packages.ts` 既有模式）、`pages/SettingsPage.vue`（错误 alert + 重试）；测试补 reject 分支与缺失 mock。

**Tech Stack:** Rust（Tauri 2）、Vue 3 + TypeScript + Naive UI + Pinia + Vitest。无新增依赖。

## Global Constraints

- 契约不变：命令名/参数/返回字段（snake_case、camelCase 转换）一律不动
- UI 文案全中文；代码不加注释；不加依赖
- 后端验证门：`cargo test` 全绿；前端：`pnpm test` 全绿、`pnpm build`（vue-tsc）无错
- 修复范围仅限推荐集；**不做**（记录为已知问题）：config 并发锁、更新匹配语义调整、索引缓存失效、`set_proxy` 关键字支持、BOM 处理
- 初始 BASE 提交：`3fd7d46`
- Windows 环境：Rust 命令前先 `export PATH="$USERPROFILE/.cargo/bin:$PATH"`

---

### Task 1: 后端桶名校验 + config 非对象备份

**Files:**
- Modify: `src-tauri/src/commands/mirrors.rs`（新增 `validate_bucket_name` 并接入 `set_bucket_source`，文件尾加测试模块）
- Modify: `src-tauri/src/domain/configfile.rs`（重写 `set_key` 备份判定，追加测试）
- Test: 上述两文件内 `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `fn validate_bucket_name(bucket: &str) -> Result<(), AppError>`（私有，拒绝空、`.`、`..`、`/`、`\`、`:`、`*`、`?`、`"`、`<`、`>`、`|`、控制字符）
- `set_key(path: &Path, key: &str, value: Option<Value>) -> Result<(), AppError>` 签名不变

- [ ] **Step 1: 写失败测试**

`src-tauri/src/domain/configfile.rs` 的 `mod tests` 内追加：

```rust
    #[test]
    fn set_key_backs_up_valid_non_object_json() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, "[1,2]").unwrap();
        set_key(&path, "proxy", Some(serde_json::json!("127.0.0.1:7890"))).unwrap();
        assert!(tmp.path().join("config.json.bak").is_file());
        assert_eq!(load(&path).get("proxy").and_then(|x| x.as_str()), Some("127.0.0.1:7890"));
    }

    #[test]
    fn set_key_keeps_object_file_without_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"scoop_branch":"master"}"#).unwrap();
        set_key(&path, "proxy", Some(serde_json::json!("127.0.0.1:7890"))).unwrap();
        assert!(!tmp.path().join("config.json.bak").is_file());
        assert_eq!(load(&path).get("scoop_branch").and_then(|x| x.as_str()), Some("master"));
    }
```

`src-tauri/src/commands/mirrors.rs` 文件尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_bucket_name_rejects_traversal_and_separators() {
        assert!(validate_bucket_name("main").is_ok());
        assert!(validate_bucket_name("my-bucket_2").is_ok());
        for bad in ["", ".", "..", "../main", "a/b", "a\\b", "C:\\scoop", "a:b", "a*b", "a\nb"] {
            assert!(validate_bucket_name(bad).is_err(), "{bad:?} 应被拒绝");
        }
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib configfile::`（`validate_bucket_name` 未定义 → 编译失败即红；configfile 两条新测试断言失败）
Expected: FAIL / compile error

- [ ] **Step 3: 实现**

`src-tauri/src/domain/configfile.rs` 用以下实现替换现有 `set_key`（第 31-63 行）：

```rust
pub fn set_key(path: &Path, key: &str, value: Option<Value>) -> Result<(), AppError> {
    let text = std::fs::read_to_string(path).ok();
    let trimmed = text.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let parsed = trimmed.and_then(|t| serde_json::from_str::<Value>(t).ok());
    if trimmed.is_some() && parsed.as_ref().map(|v| !v.is_object()).unwrap_or(true) {
        let _ = std::fs::copy(path, path.with_extension("json.bak"));
    }
    let mut current = match parsed {
        Some(Value::Object(map)) => Value::Object(map),
        _ => Value::Object(Default::default()),
    };
    let obj = current.as_object_mut().expect("上面已保证是对象");
    let entries = std::mem::take(obj);
    for (k, v) in entries {
        obj.insert(k.to_ascii_lowercase(), v);
    }
    match value {
        Some(v) => {
            obj.insert(key.to_ascii_lowercase(), v);
        }
        None => {
            obj.remove(&key.to_ascii_lowercase());
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&current)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
```

`src-tauri/src/commands/mirrors.rs` 在 `set_bucket_source` 之前加：

```rust
fn validate_bucket_name(bucket: &str) -> Result<(), AppError> {
    let invalid = bucket.is_empty()
        || bucket == "."
        || bucket == ".."
        || bucket
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control());
    if invalid {
        return Err(AppError::Other(format!("桶名称非法: {bucket}")));
    }
    Ok(())
}
```

并在 `set_bucket_source` 中 `resolve_active_root` 之后、`root.join` 之前插入：

```rust
    validate_bucket_name(&bucket)?;
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 全绿（101 + 新增 3 = 104 passed）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/commands/mirrors.rs src-tauri/src/domain/configfile.rs
git commit -m "fix: 桶名校验与 config 非对象备份"
```

---

### Task 2: AccelerationSection 出口未知提示

**Files:**
- Modify: `src/components/settings/AccelerationSection.vue`
- Test: `src/__tests__/settings-sections.spec.ts`

**Interfaces:**
- Consumes: `ProxyTest { exit_info, in_china: boolean|null, proxy_reachable, proxy_info }`
- Produces: 模板改用 `exitAlertType`/`exitHint` 计算属性；`in_china === null` 显示中性提示「未获取到出口信息，将按默认策略处理」

- [ ] **Step 1: 写失败测试**

`src/__tests__/settings-sections.spec.ts` 的 `describe("设置区块")` 内追加：

```ts
  it("加速区块出口未知时不误报可直连", async () => {
    mocks.testUrlProxy.mockResolvedValue({
      exit_info: null,
      in_china: null,
      proxy_reachable: false,
      proxy_info: null,
    });
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    const button = wrapper.findAll("button").find((b) => b.text().includes("连通测试"));
    await button!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("未获取到出口信息");
    expect(wrapper.text()).not.toContain("可直连");
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/settings-sections.spec.ts`
Expected: FAIL（当前 `in_china` 为 null 走 false 分支，渲染「不在中国，可直连」）

- [ ] **Step 3: 实现**

`src/components/settings/AccelerationSection.vue` 模板第 15-16 行改为：

```vue
    <n-alert v-if="testResult" :type="exitAlertType" size="small" class="alert">
      <div>出口 IP：{{ testResult.exit_info ?? "未获取到" }}（{{ exitHint }}）</div>
```

`<script setup>` 中 `const testResult = ref<ProxyTest | null>(null);` 之后追加：

```ts
const exitAlertType = computed(() => {
  if (testResult.value?.in_china === true) return "warning";
  if (testResult.value?.in_china === false) return "success";
  return "default";
});
const exitHint = computed(() => {
  if (testResult.value?.in_china === true) return "位于中国，建议启用代理";
  if (testResult.value?.in_china === false) return "不在中国，可直连";
  return "未获取到出口信息，将按默认策略处理";
});
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/settings-sections.spec.ts`
Expected: PASS（7 + 1 = 8 tests）

- [ ] **Step 5: 提交**

```bash
git add src/components/settings/AccelerationSection.vue src/__tests__/settings-sections.spec.ts
git commit -m "fix(ui): 出口未知时不再误报可直连"
```

---

### Task 3: apps.load 陈旧响应防护

**Files:**
- Modify: `src/stores/apps.ts`
- Test: `src/__tests__/stores.spec.ts`

**Interfaces:**
- Produces: `useAppsStore` state 增加 `seq: 0`；`load()` 内 `const seq = ++this.seq`，仅在 `seq === this.seq` 时写 `apps/updates/selectedName/loading/error`

- [ ] **Step 1: 写失败测试**

`src/__tests__/stores.spec.ts` 的 `describe("stores")` 内（`apps.load` 相关测试附近）追加：

```ts
  it("apps.load 旧响应不覆盖新响应", async () => {
    let resolveFirst!: (v: unknown) => void;
    let resolveSecond!: (v: unknown) => void;
    mocks.listInstalledApps
      .mockImplementationOnce(() => new Promise((r) => (resolveFirst = r)))
      .mockImplementationOnce(() => new Promise((r) => (resolveSecond = r)));
    mocks.listUpdates.mockResolvedValue([]);
    const store = useAppsStore();
    const first = store.load();
    const second = store.load();
    resolveSecond([{ ...appFixture, name: "7zip" }]);
    await second;
    resolveFirst([{ ...appFixture, name: "git" }]);
    await first;
    expect(store.apps.map((a) => a.name)).toEqual(["7zip"]);
    expect(store.selectedName).toBe("7zip");
    expect(store.loading).toBe(false);
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/stores.spec.ts`
Expected: FAIL（旧响应 `git` 覆盖 `7zip`）

- [ ] **Step 3: 实现**

`src/stores/apps.ts` state 增加 `seq`：

```ts
  state: () => ({
    apps: [] as InstalledApp[],
    updates: {} as Record<string, UpdateInfo>,
    selectedName: null as string | null,
    loading: false,
    error: null as string | null,
    seq: 0,
  }),
```

`load()` 替换为：

```ts
    async load() {
      const seq = ++this.seq;
      this.loading = true;
      this.error = null;
      try {
        const apps = await listInstalledApps();
        if (seq !== this.seq) return;
        this.apps = apps;
        try {
          const list = await listUpdates();
          if (seq !== this.seq) return;
          this.updates = Object.fromEntries(list.map((u) => [u.name, u]));
        } catch {
          if (seq === this.seq) this.updates = {};
        }
        if (!this.apps.some((a) => a.name === this.selectedName)) {
          this.selectedName = this.apps[0]?.name ?? null;
        }
      } catch (e) {
        if (seq !== this.seq) return;
        this.apps = [];
        this.error = String(e);
      } finally {
        if (seq === this.seq) this.loading = false;
      }
    },
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/stores.spec.ts`
Expected: PASS（既有 apps.load 用例 + 新用例全绿）

- [ ] **Step 5: 提交**

```bash
git add src/stores/apps.ts src/__tests__/stores.spec.ts
git commit -m "fix(ui): apps.load 防陈旧响应覆盖"
```

---

### Task 4: SettingsPage 镜像加载失败可见并可重试

**Files:**
- Modify: `src/pages/SettingsPage.vue`
- Test: `src/__tests__/app-components.spec.ts`（同时补齐 `listUpdates`/`getMirrorInfo` mock）

**Interfaces:**
- Produces: `loadMirror()` 失败时 `mirrorError` 有值；模板错误 alert 含「镜像信息加载失败：{msg}」与「重试」按钮

- [ ] **Step 1: 写失败测试与 mock 修正**

`src/__tests__/app-components.spec.ts`：

1) 顶部 hoisted mock 区（第 16-29 行附近）改为：

```ts
const listInstalledApps = vi.hoisted(() => vi.fn());
const listUpdates = vi.hoisted(() => vi.fn().mockResolvedValue([]));
const getScoopConfig = vi.hoisted(() => vi.fn());
const getMirrorInfo = vi.hoisted(() => vi.fn());
const runScoop = vi.hoisted(() => vi.fn());
const otherCommands = vi.hoisted(() => ({
  detectRoots: vi.fn(),
  getActiveRoot: vi.fn(),
  setActiveRoot: vi.fn(),
  addRoot: vi.fn(),
  listBuckets: vi.fn(),
  cancelTask: vi.fn(),
  updatePackage: vi.fn(),
  uninstallPackage: vi.fn(),
  setHold: vi.fn(),
}));

vi.mock("../api/commands", () => ({
  ...otherCommands,
  listInstalledApps,
  listUpdates,
  getScoopConfig,
  getMirrorInfo,
  runScoop,
  fetchIcon,
}));
```

2) 类型导入改为：

```ts
import type { InstalledApp, MirrorInfo, RootInfo } from "../api/types";
```

3) `beforeEach` 改为：

```ts
beforeEach(() => {
  setActivePinia(createPinia());
  fetchIcon.mockReset().mockResolvedValue(null);
  listUpdates.mockReset().mockResolvedValue([]);
  getMirrorInfo.mockReset().mockResolvedValue(mirrorFixture);
});
```

4) `beforeEach` 之前加 fixture：

```ts
const mirrorFixture: MirrorInfo = {
  scoop_repo: "https://gitee.com/scoop-installer/scoop",
  scoop_remote: "https://gitee.com/scoop-installer/scoop.git",
  presets: {
    official: "https://github.com/ScoopInstaller/scoop",
    gitee: "https://gitee.com/scoop-installer/scoop",
    proxy: "https://scoop.201704.xyz/https://github.com/ScoopInstaller/scoop",
  },
  url_proxy: null,
  default_url_proxy: "https://scoop.201704.xyz",
  proxy: null,
  buckets: [],
  aria2_installed: false,
  aria2_enabled: true,
  aria2_split: null,
  aria2_max_connection: null,
};
```

5) `describe("SettingsPage")` 内追加：

```ts
  it("镜像信息加载失败时展示错误并可重试", async () => {
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
    getScoopConfig.mockResolvedValue({
      file_path: "C:\\scoop\\config.json",
      portable: false,
      values: {},
    });
    getMirrorInfo.mockRejectedValueOnce("无法读取 scoop 仓库地址");
    const wrapper = mount(SettingsPage, { global: { plugins: [pinia] } });
    await flushPromises();
    expect(wrapper.text()).toContain("镜像信息加载失败");
    expect(wrapper.text()).toContain("无法读取 scoop 仓库地址");

    const retry = wrapper.findAll("button").find((b) => b.text().includes("重试"));
    await retry!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).not.toContain("镜像信息加载失败");
    expect(wrapper.text()).toContain("镜像源");
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/app-components.spec.ts`
Expected: FAIL（当前失败静默，无「镜像信息加载失败」文本与重试按钮）

- [ ] **Step 3: 实现**

`src/pages/SettingsPage.vue`：

1) `naive-ui` 导入加 `NAlert`：

```ts
import { NAlert, NButton, NDescriptions, NDescriptionsItem } from "naive-ui";
```

2) `const mirror = ref<MirrorInfo | null>(null);` 之后加：

```ts
const mirrorError = ref<string | null>(null);
```

3) `loadMirror()` 替换为：

```ts
async function loadMirror() {
  mirrorError.value = null;
  if (!roots.active) {
    mirror.value = null;
    return;
  }
  try {
    mirror.value = await getMirrorInfo();
  } catch (e) {
    mirror.value = null;
    mirrorError.value = String(e);
  }
}
```

4) 模板在 `<MirrorSection ... />` 之前插入：

```vue
    <n-alert v-if="mirrorError" type="error" size="small" class="mirror-error">
      <div>镜像信息加载失败：{{ mirrorError }}</div>
      <n-button size="tiny" class="retry" @click="loadMirror">重试</n-button>
    </n-alert>
```

5) 样式追加：

```css
.mirror-error {
  margin-top: 12px;
}
.retry {
  margin-top: 6px;
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/app-components.spec.ts`
Expected: PASS（既有 SettingsPage/InstalledPage 用例 + 新用例全绿）

- [ ] **Step 5: 提交**

```bash
git add src/pages/SettingsPage.vue src/__tests__/app-components.spec.ts
git commit -m "fix(ui): 镜像信息加载失败可见并可重试"
```

---

### Task 5: 错误分支测试补齐与 mock 契约修正

**Files:**
- Modify: `src/__tests__/settings-sections.spec.ts`（删死 mock；补 reject 分支）
- Modify: `src/__tests__/app-ops.spec.ts`（补 `listUpdates`/`getMirrorInfo` mock）

**Interfaces:**
- Consumes: Task 2-4 的实现
- Produces: 组件错误分支断言（`message.error` 被调用、失败不 emit `changed`、loading 复位）

- [ ] **Step 1: 写测试（预期先失败/部分失败）**

`src/__tests__/settings-sections.spec.ts`：

1) 删除 aria2 用例中的死 mock 行（原第 148 行）：

```ts
    mocks.getMirrorInfo.mockResolvedValue(info({ aria2_installed: true }));
```

并同步从顶部 `mocks` hoisted 对象中删除 `getMirrorInfo: vi.fn(),`（组件不调用该命令）。

2) `describe("设置区块")` 内追加：

```ts
  it("镜像区块切换失败时提示错误且不通知刷新", async () => {
    mocks.setBucketSource.mockRejectedValueOnce("该仓库不是官方 GitHub/gitee 已知仓库，无法一键切换源");
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    const rows = wrapper.findAll(".bucket-row");
    rows[0].findComponent({ name: "Select" }).vm.$emit("update:value", "official");
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(wrapper.emitted("changed")).toBeFalsy();
  });

  it("scoop 源切换失败时提示错误且不通知刷新", async () => {
    mocks.setScoopSource.mockRejectedValueOnce("切换源失败: git 执行失败");
    const wrapper = mount(MirrorSection, { props: { info: info() } });
    wrapper.findComponent({ name: "Select" }).vm.$emit("update:value", "official");
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(wrapper.emitted("changed")).toBeFalsy();
  });

  it("连通测试失败时复位 loading 并提示错误", async () => {
    mocks.testUrlProxy.mockRejectedValueOnce("连通测试失败");
    const wrapper = mount(AccelerationSection, { props: { info: info() } });
    const button = wrapper.findAllComponents({ name: "Button" }).find((b) => b.text().includes("连通测试"));
    await button!.trigger("click");
    await flushPromises();
    expect(button!.props("loading")).toBe(false);
    expect(messageError).toHaveBeenCalled();
  });

  it("aria2 安装失败时提示错误且不通知刷新", async () => {
    const wrapper = mount(Aria2Section, { props: { info: info() } });
    const install = wrapper.findAll("button").find((b) => b.text().includes("安装 aria2"));
    await install!.trigger("click");
    await flushPromises();
    const taskId = mocks.installPackage.mock.calls[0][4];
    useLogsStore().end({ taskId, code: 1, success: false });
    await flushPromises();
    expect(messageError).toHaveBeenCalled();
    expect(wrapper.emitted("changed")).toBeFalsy();
  });
```

`src/__tests__/app-ops.spec.ts` 的 hoisted mocks 对象内追加：

```ts
  listUpdates: vi.fn().mockResolvedValue([]),
  getMirrorInfo: vi.fn().mockResolvedValue(null),
```

- [ ] **Step 2: 运行测试**

Run: `pnpm vitest run src/__tests__/settings-sections.spec.ts src/__tests__/app-ops.spec.ts`
Expected: 新增用例应 PASS（实现已在 Task 2-4 就位；若失败说明实现有缺口，回到对应任务修复）

- [ ] **Step 3: 全量验证**

Run: `pnpm test && pnpm build`
Expected: 全部 PASS（113 + 约 8 = 约 121），`vue-tsc` 无错

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 104 passed

- [ ] **Step 4: 提交**

```bash
git add src/__tests__/settings-sections.spec.ts src/__tests__/app-ops.spec.ts
git commit -m "test(ui): 补 M4 错误分支与 mock 契约"
```

---

## Self-Review 记录

- 规格覆盖：评审推荐集 6 项逐一对应——桶名校验/非对象备份（Task 1）、null 误报（Task 2）、load 竞态（Task 3）、loadMirror 静默（Task 4）、测试盲区与 mock 契约（Task 4 Step 1 的 app-components mock、Task 5）；不做项已在 Global Constraints 列明。
- 占位符扫描：无 TBD；所有步骤含完整代码与精确命令。
- 类型一致性：`seq` 模式与 `stores/packages.ts` 一致；`exitAlertType` 返回 `"warning"|"success"|"default"` 与 NAlert type 兼容；`mirrorError: string | null` 与模板一致；`validate_bucket_name` 在 `set_bucket_source` 中调用且测试同模块引用。
- 已知取舍：`set_key` 备份会覆盖旧 `.bak`（与原行为一致）；`validate_bucket_name` 允许空格/中文（仅拒绝分隔符与危险字符）；竞态防护仅覆盖 `load()`，不扩展其他 store。
- 风险：Task 4 的 mock 补齐会让既有 SettingsPage 用例渲染三个设置区块（依赖 pinia/useMessage mock，已具备）；Task 5 的 `findAllComponents({ name: "Button" })` 依赖 naive-ui 组件名（既有测试已用 `{ name: "Select" }`，同机制）。
