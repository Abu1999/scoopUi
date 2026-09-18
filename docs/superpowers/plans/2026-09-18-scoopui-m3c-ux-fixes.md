# ScoopUI M3c（UX 修复：列表宽度 / 防连点 / 已知桶下拉 / 维护页说明）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修 4 个用户体验问题：已安装列表过窄、危险操作确认可连点、桶名需手输（已知桶不可选）、维护页功能含义不清。

**Architecture:** 后端新增 `list_known_buckets` 命令（`scoop bucket known` 捕获输出 → 纯函数解析）；前端桶页名称改可下拉可自定义的 `n-select`；所有操作入口加 busy 防重入并禁用确认按钮；维护页每卡展示实际命令与说明。

**Tech Stack:** Rust（Tauri 2）、Vue 3 + Naive UI + Pinia + Vitest。无新增依赖。

## Global Constraints

- 新命令契约：`list_known_buckets() -> Result<Vec<String>, AppError>`；失败 reject 中文；仅 stdout 行参与解析；名称过滤 `[a-z0-9_-]`、去重、保序
- 前端只经 `src/api/commands.ts`；UI 全中文；代码不加注释；不加依赖
- 防重入语义：`logs.busy` 期间所有操作处理函数立即返回；确认弹窗「确认」按钮 `positiveButtonProps.disabled = busy`
- 验证门：`cargo test` 全绿、`pnpm test` 全绿、`pnpm build` 无错
- 初始 BASE 提交：`6824ada`

---

### Task 1: 后端 `list_known_buckets` + 前端命令包装

**Files:**
- Modify: `src-tauri/src/domain/scoopcmd.rs`（`capture`）
- Modify: `src-tauri/src/domain/scoopops.rs`（`parse_known_buckets`）
- Modify: `src-tauri/src/commands/buckets.rs`（命令）
- Modify: `src-tauri/src/lib.rs`（注册）
- Modify: `src/api/commands.ts`、`src/__tests__/commands.spec.ts`

**Interfaces:**
- Produces:
  - `scoopcmd::capture(cmd: &ScoopCommand) -> Result<String, AppError>`（收集 stdout 行，非零退出返回中文错误）
  - `scoopops::parse_known_buckets(output: &str) -> Vec<String>`
  - 命令 `list_known_buckets() -> string[]`；前端 `listKnownBuckets()`

- [ ] **Step 1: 写失败测试**

`scoopops.rs` 测试模块追加：

```rust
    #[test]
    fn parses_known_buckets_filtering_noise() {
        let out = "main\n extras \n\nJAVA\nmain\nWarning: something\nphp\nbad name\n";
        assert_eq!(parse_known_buckets(out), vec!["main", "extras", "java", "php"]);
    }
```

`scoopcmd.rs` 测试模块追加：

```rust
    #[test]
    fn capture_collects_stdout_only() {
        let cmd = ScoopCommand {
            program: "cmd.exe".to_string(),
            args: vec![
                "/C".to_string(),
                "(echo hello & echo warn 1>&2)".to_string(),
            ],
            envs: vec![],
            display: "capture-test".to_string(),
        };
        let out = capture(&cmd).unwrap();
        assert!(out.contains("hello"));
        assert!(!out.contains("warn"));
    }

    #[test]
    fn capture_fails_on_nonzero_exit() {
        let cmd = ScoopCommand {
            program: "cmd.exe".to_string(),
            args: vec!["/C".to_string(), "exit /b 3".to_string()],
            envs: vec![],
            display: "capture-fail".to_string(),
        };
        let err = capture(&cmd).unwrap_err();
        assert!(err.to_string().contains("退出码"));
    }
```

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test capture parses_known --lib` → 红（函数不存在）。

- [ ] **Step 2: 实现 capture 与解析**

`scoopcmd.rs` 在 `stream_child` 之后加：

```rust
pub fn capture(cmd: &ScoopCommand) -> Result<String, AppError> {
    let spawned = spawn(cmd)?;
    let collected = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let sink = collected.clone();
    let code = stream_child(spawned, move |l| {
        if l.stream == "stdout" {
            if let Ok(mut s) = sink.lock() {
                s.push_str(&l.line);
                s.push('\n');
            }
        }
    })?;
    if code != Some(0) {
        let code_text = code.map(|c| c.to_string()).unwrap_or_else(|| "未知".to_string());
        return Err(AppError::Other(format!("命令执行失败（退出码 {code_text}）：{}", cmd.display)));
    }
    Ok(collected.lock().map(|s| s.clone()).unwrap_or_default())
}
```

`scoopops.rs` 末尾加：

```rust
pub fn parse_known_buckets(output: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in output.lines() {
        let name = line.trim().to_ascii_lowercase();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}
```

- [ ] **Step 3: 命令与注册**

`src-tauri/src/commands/buckets.rs` 追加（保留现有 `list_buckets`）：

```rust
use crate::commands::{context, roots::resolve_active_root};
use crate::domain::root;
use crate::domain::{scoopcmd, scoopops};

#[tauri::command]
pub async fn list_known_buckets(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<String>, crate::error::AppError> {
    let ctx = context();
    let root_path = resolve_active_root(&ctx, &state).ok_or(crate::error::AppError::NoRoot)?;
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    let cmd = scoopcmd::build(
        &root_path,
        Some(&global),
        &["bucket".to_string(), "known".to_string()],
    )?;
    let output = tauri::async_runtime::spawn_blocking(move || scoopcmd::capture(&cmd))
        .await
        .map_err(|e| crate::error::AppError::Other(format!("已知桶列表获取失败: {e}")))??;
    Ok(scoopops::parse_known_buckets(&output))
}
```

（实现者可将其整理为与文件既有 import 风格一致；`lib.rs` 注册 `commands::buckets::list_known_buckets`。）

`src/api/commands.ts` 末尾加：

```ts
export const listKnownBuckets = () => invoke<string[]>("list_known_buckets");
```

`commands.spec.ts` 追加断言（import `listKnownBuckets`）：

```ts
    await listKnownBuckets();
    expect(invokeMock).toHaveBeenLastCalledWith("list_known_buckets", undefined);
```

- [ ] **Step 4: 验证与提交**

Run: `cargo test && cargo check`；`pnpm vitest run src/__tests__/commands.spec.ts && pnpm test && pnpm build`
Expected: 全绿（cargo 预计 83，pnpm 预计 94）。

```bash
git add src-tauri/src/domain/scoopcmd.rs src-tauri/src/domain/scoopops.rs src-tauri/src/commands/buckets.rs src-tauri/src/lib.rs src/api/commands.ts src/__tests__/commands.spec.ts
git commit -m "feat: list_known_buckets 命令（capture + 解析）与前端包装"
```

---

### Task 2: 前端 4 项 UX 优化

**Files:**
- Modify: `src/pages/InstalledPage.vue`（宽度 280→340）
- Modify: `src/components/app/AppDetail.vue`、`src/components/store/PackageDetail.vue`、`src/pages/BucketsPage.vue`、`src/pages/MaintenancePage.vue`（防连点 + 已知桶下拉 + 维护说明）
- Test: `src/__tests__/app-ops.spec.ts`、`src/__tests__/store-page.spec.ts`、`src/__tests__/buckets-page.spec.ts`、`src/__tests__/maintenance-page.spec.ts`

**Interfaces:**
- Consumes: Task 1 的 `listKnownBuckets`
- Produces: 见各步

- [ ] **Step 1: 已安装列表宽度**

`src/pages/InstalledPage.vue` 样式 `.side` 的 `width: 280px;` → `width: 340px;`。

- [ ] **Step 2: 防连点（测试先行）**

各页面处理函数入口加 `if (busy.value) return;`；确认弹窗加 `:positive-button-props="{ disabled: busy, type: 'error' }"`（维护页去掉 `type: 'error'`，用 `{ disabled: busy }`）。

测试追加：

`app-ops.spec.ts`：

```ts
  it("任务运行中确认卸载不会再次发起", async () => {
    const wrapper = mount(AppDetail, { props: { app: git } });
    useLogsStore().start("t1", "更新 git");
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.uninstallPackage).not.toHaveBeenCalled();
  });
```

`buckets-page.spec.ts`：

```ts
  it("任务运行中确认删除不会再次发起", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    useLogsStore().start("t1", "更新");
    await wrapper.findComponent(NPopconfirm).vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.bucketRemove).not.toHaveBeenCalled();
  });
```

`maintenance-page.spec.ts`：

```ts
  it("任务运行中确认操作不会再次发起", async () => {
    const wrapper = mount(MaintenancePage);
    useLogsStore().start("t1", "维护");
    await wrapper.findAllComponents(NPopconfirm)[0].vm.$emit("positive-click");
    await flushPromises();
    expect(mocks.runMaintenance).not.toHaveBeenCalled();
  });
```

- [ ] **Step 3: 桶名下拉（已知桶可选）**

`BucketsPage.vue`：名称输入替换为

```vue
      <n-select
        v-model:value="newName"
        size="small"
        filterable
        tag
        clearable
        :options="nameOptions"
        placeholder="桶名称（可选已知桶或直接输入）"
        class="name"
      />
```

script：

```ts
import { computed, onMounted, ref } from "vue";
import { NAlert, NButton, NPopconfirm, NSelect, NTable, useMessage } from "naive-ui";
import { bucketAdd, bucketRemove, listKnownBuckets, updatePackage } from "../api/commands";
...
const newName = ref<string | null>(null);
const known = ref<string[]>([]);
const nameOptions = computed(() => known.value.map((n) => ({ label: n, value: n })));
...
onMounted(async () => {
  try {
    known.value = await listKnownBuckets();
  } catch {
    known.value = [];
  }
  await reload();
});
```

`doAdd`/`canAdd` 改用 `(newName.value ?? "").trim()`；成功后 `newName.value = null`。

测试更新：`buckets-page.spec.ts` mocks 增加 `listKnownBuckets: vi.fn().mockResolvedValue(["extras", "java", "games"])`；既有「添加桶」用例改为通过 `findComponent(NSelect).vm.$emit("update:value", "extras")` 设置名称；新增断言已知桶选项已加载：

```ts
  it("下拉展示已知桶并可直接选择", async () => {
    const wrapper = mount(BucketsPage);
    await flushPromises();
    const select = wrapper.findComponent(NSelect);
    expect(select.props("options")).toEqual([
      { label: "extras", value: "extras" },
      { label: "java", value: "java" },
      { label: "games", value: "games" },
    ]);
    select.vm.$emit("update:value", "games");
    await flushPromises();
    const button = wrapper.findAll("button").find((b) => b.text().includes("添加桶"));
    await button!.trigger("click");
    await flushPromises();
    expect(mocks.bucketAdd).toHaveBeenCalledWith("games", null, expect.any(String));
  });
```

- [ ] **Step 4: 维护页说明**

`MaintenancePage.vue`：
- `MaintenanceOp` 增加 `command: string`；5 项依次为 `scoop cache rm -a`、`scoop cleanup -a`、`scoop cleanup -a -g`、`scoop checkup`、`scoop reset -a`
- 卡片模板在 desc 后加 `<code class="cmd">{{ op.command }}</code>`
- 顶部提示改为：`操作为真实执行：点击按钮并按提示确认后运行，进度与结果见底部任务日志抽屉（运行中可取消）。`
- 确认文案带命令，如：`确定清空下载缓存（scoop cache rm -a）？`
- 样式：`.cmd { font-family: Consolas, monospace; font-size: 12px; color: #555; background: #f7f7f9; border-radius: 3px; padding: 2px 6px; }`

测试追加：

```ts
  it("展示实际命令与操作指引", () => {
    const wrapper = mount(MaintenancePage);
    const text = wrapper.text();
    expect(text).toContain("scoop cache rm -a");
    expect(text).toContain("scoop reset -a");
    expect(text).toContain("底部任务日志");
  });
```

- [ ] **Step 5: 验证与提交**

Run: `pnpm vitest run src/__tests__/app-ops.spec.ts src/__tests__/store-page.spec.ts src/__tests__/buckets-page.spec.ts src/__tests__/maintenance-page.spec.ts` 全绿；`pnpm test && pnpm build` 全绿。

```bash
git add src/pages/InstalledPage.vue src/components/app/AppDetail.vue src/components/store/PackageDetail.vue src/pages/BucketsPage.vue src/pages/MaintenancePage.vue src/__tests__/app-ops.spec.ts src/__tests__/store-page.spec.ts src/__tests__/buckets-page.spec.ts src/__tests__/maintenance-page.spec.ts
git commit -m "feat(ui): 列表加宽、操作防连点、已知桶下拉与维护页说明"
```

---

## Self-Review 记录

- 覆盖：用户 4 条反馈 → Task 1（已知桶数据）+ Task 2（宽度/防连点/下拉/说明）；`update`/`hold` 等操作入口同样加防重入（超出反馈范围的同类加固，与小节目标一致）。
- 占位符扫描：无 TBD；全部给出代码。
- 类型一致性：`listKnownBuckets` 在 Task 1 定义、Task 2 使用；`positiveButtonProps` 为 naive-ui 既有 API（已确认 d.ts 存在）。
- 已知取舍：`n-select` 使用 `tag` 允许自定义名称（覆盖非已知桶/自定义仓库场景）；已知桶列表获取失败时下拉为空、仍可手输。
