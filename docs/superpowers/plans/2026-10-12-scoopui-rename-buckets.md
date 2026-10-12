# ScoopUI 术语替换（桶 → 软件源）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把用户可见处的「桶」（Scoop bucket）全部替换为「软件源」，前后端文案与测试同步；命令名与代码标识符不变。

**Architecture:** 纯文案替换，分后端（3 个文件 + 2 个测试）与前端（7 个源文件 + 7 个测试文件）两个任务，最后一个验证任务确认 `grep -r 桶 src src-tauri/src` 为 0 且全部门禁通过。

**Tech Stack:** Rust（Tauri 2）、Vue 3 + TypeScript + Naive UI + Vitest。无新增依赖。

**Spec:** `docs/superpowers/specs/2026-10-12-scoopui-m5b-m7-remaining-design.md` §1

## Global Constraints

- 只改用户可见中文文案与对应测试断言/用例名；命令名（`bucket_add` 等）、类型名、变量名、`docs/` 历史文档一律不动
- 代码不加注释；不加依赖
- 完成标准：`grep -rn 桶 src src-tauri/src` 输出为空
- 验证门：`cargo test` 全绿（120 passed）；`pnpm test` 全绿（139 passed）；`pnpm build`（vue-tsc）无错
- 初始 BASE 提交：`9045e86`
- Windows 环境：Rust 命令前先 `export PATH="$USERPROFILE/.cargo/bin:$PATH"`

---

### Task 1: 后端用户可见文案替换

**Files:**
- Modify: `src-tauri/src/domain/scoopops.rs`（第 28、89、98 行的 `"桶名"` + 测试模块新增用例）
- Modify: `src-tauri/src/commands/mirrors.rs`（第 120、136、139 行错误消息 + 既有测试追加断言）
- Modify: `src-tauri/src/commands/buckets.rs`（第 29 行错误消息）
- Test: 上述两个文件的 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: 无
- Produces: 错误消息文案 `软件源名称非法: {bucket}`、`软件源不存在: {bucket}`、`软件源不是 git 仓库，无法切换源: {bucket}`、`已知软件源列表获取失败: {e}`、`{kind}不能为空…`（kind=`软件源名称`）

- [ ] **Step 1: 写失败测试**

`src-tauri/src/domain/scoopops.rs` 的 `mod tests` 内（`bucket_add_and_remove` 之后）追加：

```rust
    #[test]
    fn bucket_errors_use_software_source_wording() {
        let err = bucket_add_args("", None).unwrap_err().to_string();
        assert!(err.contains("软件源名称"), "实际: {err}");
        let err = bucket_remove_args("a b").unwrap_err().to_string();
        assert!(err.contains("软件源名称"), "实际: {err}");
    }
```

`src-tauri/src/commands/mirrors.rs` 的 `validate_bucket_name_rejects_traversal_and_separators` 测试末尾追加：

```rust
        let err = validate_bucket_name("a/b").unwrap_err().to_string();
        assert!(err.contains("软件源名称非法"), "实际: {err}");
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test --lib`
Expected: FAIL（断言收到「桶名」/「桶名称非法」，不含「软件源…」）

- [ ] **Step 3: 实现**

`src-tauri/src/domain/scoopops.rs` 三处 `"桶名"` 改为 `"软件源名称"`：

```rust
    let bucket = optional(bucket, "软件源名称")?;
```

```rust
pub fn bucket_add_args(name: &str, repo: Option<&str>) -> Result<Vec<String>, AppError> {
    let name = require(name, "软件源名称")?;
```

```rust
pub fn bucket_remove_args(name: &str) -> Result<Vec<String>, AppError> {
    let name = require(name, "软件源名称")?;
```

`src-tauri/src/commands/mirrors.rs` 三处错误消息：

```rust
        return Err(AppError::Other(format!("软件源名称非法: {bucket}")));
```

```rust
        return Err(AppError::Other(format!("软件源不存在: {bucket}")));
```

```rust
        .ok_or_else(|| AppError::Other(format!("软件源不是 git 仓库，无法切换源: {bucket}")))?;
```

`src-tauri/src/commands/buckets.rs` 第 29 行：

```rust
        .map_err(|e| AppError::Other(format!("已知软件源列表获取失败: {e}")))??;
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 120 passed（119 + 1）

再确认后端无残留：`grep -rn 桶 src-tauri/src` 输出为空

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/domain/scoopops.rs src-tauri/src/commands/mirrors.rs src-tauri/src/commands/buckets.rs
git commit -m "refactor: 后端文案桶改为软件源"
```

---

### Task 2: 前端文案替换与测试同步

**Files:**
- Modify: `src/components/layout/SideBar.vue:19`
- Modify: `src/pages/BucketsPage.vue`（第 11、14、15、16、43、103、114、121 行）
- Modify: `src/components/settings/MirrorSection.vue`（第 25、100 行）
- Modify: `src/components/store/PackageDetail.vue`（第 13、37 行）
- Modify: `src/pages/ProfilesPage.vue`（第 19、55、64、76、90、134 行）
- Modify: `src/pages/SettingsPage.vue:8`
- Modify: `src/stores/profiles.ts:163`
- Test: `src/__tests__/buckets-page.spec.ts`、`layout.spec.ts`、`appshell.spec.ts`、`commands.spec.ts`、`profiles-page.spec.ts`、`profiles.spec.ts`、`settings-sections.spec.ts`

**Interfaces:**
- Consumes: Task 1 的后端文案（无直接依赖）
- Produces: 全部前端界面文案使用「软件源」

- [ ] **Step 1: 先改测试（红）**

`src/__tests__/buckets-page.spec.ts`：

1) 第 48 行用例名改为 `"添加软件源调用 bucketAdd 并刷新列表"`；
2) 第 53 行选择器改为 `'input[placeholder="仓库地址（可选，已知软件源可留空）"]'`；
3) 第 54、68、85、126 行的 `includes("添加桶")` 改为 `includes("添加软件源")`；
4) 第 64 行用例名改为 `"已知软件源仓库留空时传 null"`；
5) 第 74 行用例名改为 `"下拉展示已知软件源并可直接选择"`；
6) 第 100 行用例名改为 `"删除软件源需确认且失败提示错误"`；
7) 第 113 行用例名与第 116 行按钮文案改为 `"更新 Scoop 与软件源"`。

`src/__tests__/layout.spec.ts:54`：

```ts
    const labels = ["已安装", "商店", "软件源管理", "维护", "环境迁移", "配置集", "备份", "设置"];
```

`src/__tests__/appshell.spec.ts`：第 85 行用例名改为 `"按日志→根目录→应用→软件源的顺序串行加载"`；第 100 行改为 `"日志初始化失败仍继续加载根目录、应用与软件源"`。

`src/__tests__/commands.spec.ts:140` 用例名改为 `"M3c 已知软件源命令名符合后端契约"`。

`src/__tests__/profiles-page.spec.ts:104` 断言改为：

```ts
    expect(wrapper.text()).toContain("缺少软件源");
```

`src/__tests__/profiles.spec.ts:89` 用例名改为 `"apply 依次执行 settings→软件源→应用"`。

`src/__tests__/settings-sections.spec.ts:83` 用例名改为 `"镜像区块切换软件源并通知刷新"`。

- [ ] **Step 2: 运行测试确认失败**

Run: `pnpm vitest run src/__tests__/buckets-page.spec.ts src/__tests__/layout.spec.ts src/__tests__/profiles-page.spec.ts`
Expected: FAIL（页面仍渲染「桶…」文案，新选择器/按钮找不到；profiles 差异文本仍为「缺少桶」）

- [ ] **Step 3: 实现**

`src/components/layout/SideBar.vue:19`：

```ts
  { label: "软件源管理", key: "buckets" },
```

`src/pages/BucketsPage.vue`：

```vue
        placeholder="软件源名称（可选已知软件源或直接输入）"
```

```vue
      <n-input v-model:value="newRepo" size="small" placeholder="仓库地址（可选，已知软件源可留空）" class="repo" />
      <n-button size="small" :disabled="busy || !canAdd" @click="doAdd">添加软件源</n-button>
      <n-button size="small" :disabled="busy" @click="doUpdateAll">更新 Scoop 与软件源</n-button>
```

```vue
              确定删除软件源 {{ b.name }}？该软件源提供的应用将无法安装/更新。
```

```ts
  const record = await runTask(`添加软件源 ${name}`, (taskId) => bucketAdd(name, repo, taskId));
```

```ts
  const record = await runTask(`删除软件源 ${name}`, (taskId) => bucketRemove(name, taskId));
```

```ts
  const record = await runTask("更新 Scoop 与软件源", (taskId) => updatePackage(null, false, false, taskId));
```

`src/components/settings/MirrorSection.vue`：

```vue
          <th>软件源</th>
```

```ts
    message.success(`软件源 ${name} 已切换: ${url}`);
```

`src/components/store/PackageDetail.vue` 两处：

```vue
            版本 {{ pkg.version }} · 来自软件源 {{ pkg.bucket }}<span v-if="pkg.license"> · {{ pkg.license }}</span>
```

```vue
            来自软件源 {{ pkg.bucket }}<span v-if="globalInstall">；全局安装可能需要管理员权限</span>
```

`src/pages/ProfilesPage.vue`：

```vue
          <div class="meta">应用 {{ p.app_count }} · 软件源 {{ p.bucket_count }}</div>
```

```vue
              缺少软件源：{{ profiles.diff!.missing_buckets.map((b) => b.name).join("、") }}
```

```vue
              多余软件源（不处理）：{{ profiles.diff!.extra_buckets.join("、") }}
```

```vue
          <div class="section-title">软件源（{{ profiles.detail.buckets.length }}）</div>
```

```vue
            <span class="dim">{{ a.bucket ?? "（未知软件源）" }}</span>
```

```vue
        缺少软件源 {{ profiles.diff.missing_buckets.length }} · 缺少应用 {{ profiles.diff.missing_apps.length }} ·
```

`src/pages/SettingsPage.vue:8`：

```vue
      <n-descriptions-item label="应用 / 软件源数量">
```

`src/stores/profiles.ts:163`：

```ts
        steps.push({ key: `bucket:${b.name}`, label: `添加软件源 ${b.name}`, status: "pending" });
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm vitest run src/__tests__/buckets-page.spec.ts src/__tests__/layout.spec.ts src/__tests__/profiles-page.spec.ts src/__tests__/profiles.spec.ts src/__tests__/settings-sections.spec.ts src/__tests__/appshell.spec.ts src/__tests__/commands.spec.ts`
Expected: PASS

Run: `pnpm test`
Expected: 139 passed

前端无残留确认：`grep -rn 桶 src` 输出为空

- [ ] **Step 5: 提交**

```bash
git add src/components/layout/SideBar.vue src/pages/BucketsPage.vue src/components/settings/MirrorSection.vue src/components/store/PackageDetail.vue src/pages/ProfilesPage.vue src/pages/SettingsPage.vue src/stores/profiles.ts src/__tests__/buckets-page.spec.ts src/__tests__/layout.spec.ts src/__tests__/appshell.spec.ts src/__tests__/commands.spec.ts src/__tests__/profiles-page.spec.ts src/__tests__/profiles.spec.ts src/__tests__/settings-sections.spec.ts
git commit -m "refactor(ui): 文案桶改为软件源"
```

---

### Task 3: 全量验证

**Files:** 无新增；只做验证与收尾

- [ ] **Step 1: 全局残留检查**

Run: `grep -rn 桶 src src-tauri/src`
Expected: 输出为空（无匹配）

- [ ] **Step 2: 后端全量**

Run: `cd src-tauri && export PATH="$USERPROFILE/.cargo/bin:$PATH" && cargo test`
Expected: 120 passed、0 failed

- [ ] **Step 3: 前端全量与构建**

Run: `pnpm test`
Expected: 139 passed

Run: `pnpm build`
Expected: `vue-tsc` 无错误、vite build 成功

- [ ] **Step 4: 修复与提交（仅当验证发现问题）**

若发现问题：修复 → 回到 Step 1 重新验证 → 提交（`fix: ...`）。全部通过则无需额外提交。

---

## Self-Review 记录

- 规格覆盖：映射表逐项落到 Task 1（后端 4 类消息）与 Task 2（前端 7 文件）；§1.1 表中的「已知桶（列表）」对应 buckets.rs 消息与 BucketsPage 下拉文案（下拉 placeholder 含「已知软件源」）。
- 占位符扫描：无 TBD；所有替换给出精确旧/新文案与文件行号。
- 类型一致性：仅文案变化，无类型/接口变化；命令名与 store 方法名未动（`bucket_add`、`bucketAdd`、`updateFor` 等）。
- 已知取舍：`docs/` 与既有 specs/plans 保留「桶」术语（历史记录）；`migrations`/`Backup` 等后续新页面直接使用「软件源」文案。
