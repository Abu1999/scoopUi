# ScoopUI M0（环境与脚手架）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Windows 上装好 Tauri 2 工具链，生成 Vue 3 + TS 脚手架并让空窗口跑通，接入 Naive UI/Pinia 与首个自动化测试，为 M1 开发打好可验证的地基。

**Architecture:** 单仓库 Tauri 2 项目：`src/`（Vue 3 + TS 前端）、`src-tauri/`（Rust 后端）。M0 只搭骨架与验证通道（编译、测试、窗口进程），不写业务逻辑。

**Tech Stack:** Tauri 2、Rust stable（MSVC 工具链）、Node 24 + pnpm 11、Vue 3 + TypeScript + Vite、Naive UI、Pinia、Vitest。

## Global Constraints

- 平台：Windows 10+ x64（本机 Win10 22H2 / 19045），WebView2 已安装（149.0.4022.80）
- 包管理器：pnpm（不用 npm/yarn）；Rust 用 cargo
- UI 文案一律中文；代码标识符/注释规则遵循项目约定（实现代码默认不加注释）
- 应用标识：`dev.scoopui.app`；Cargo/包名：`scoopui`
- 应用数据目录（后续阶段使用）：`%APPDATA%\scoopui\`、图标缓存 `%LOCALAPPDATA%\scoopui\icons\`
- 不修改系统环境变量；不引入除清单中列出的依赖
- 网络事实：npmjs 0.38s、crates.io 0.63s（直连可用，无需换源）
- 文件写入使用编辑器工具，不用 shell 重定向

---

### Task 1: 工具链安装验证（Rust + MSVC）

**Files:**
- 无仓库文件改动（验证任务）

**Interfaces:**
- Consumes: 无
- Produces: 可用的 `rustc`/`cargo`（后续所有 Rust 任务依赖）

- [ ] **Step 1: 等待后台 winget 安装完成**

Run（轮询直到两个日志都出现成功字样，或 `winget list` 能查到）:

```bash
tail -5 "/c/Users/Administrator/AppData/Local/Temp/opencode/rustup-install.log"
tail -5 "/c/Users/Administrator/AppData/Local/Temp/opencode/buildtools-install.log"
winget list --id Rustlang.Rustup 2>&1 | tail -3
winget list --id Microsoft.VisualStudio.2022.BuildTools 2>&1 | tail -3
```

Expected: 两个包都显示已安装（“已成功安装” / “Successfully installed”）。

- [ ] **Step 2: 验证 rustc / cargo 可用**

Run:

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"
rustc --version && cargo --version && rustup show
```

Expected: `rustc 1.9x.x (…)`、`cargo 1.9x.x (…)`、`rustup show` 输出 `stable-x86_64-pc-windows-msvc`。

若 `rustc` 不在 PATH：把 `%USERPROFILE%\.cargo\bin` 加入当前 shell PATH 后重试；若工具链未装：`rustup toolchain install stable-x86_64-pc-windows-msvc`。

- [ ] **Step 3: 链接器冒烟测试（证明 MSVC 可用）**

Run（临时目录，用后删除）:

```bash
SMOKE="/c/Users/Administrator/AppData/Local/Temp/opencode/rust-smoke"
rm -rf "$SMOKE" && mkdir -p "$SMOKE" && cd "$SMOKE"
cargo new smoke --bin -q && cd smoke
cargo build -q 2>&1 | tail -5 && echo "BUILD_OK"
cd / && rm -rf "$SMOKE"
```

Expected: 输出 `BUILD_OK`（若报 `link.exe not found`，等 VS BuildTools 安装完成或重启 shell 再试）。

- [ ] **Step 4: 验证 Tauri CLI 可解析（不安装全局）**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
pnpm dlx @tauri-apps/cli@latest --version
```

Expected: 输出 `tauri-cli 2.x.x`。

---

### Task 2: Tauri 脚手架 + 空窗口跑通

**Files:**
- Create: `package.json`、`vite.config.ts`、`tsconfig.json`、`tsconfig.node.json`、`index.html`、`src/**`、`src-tauri/**`、`README.md`、`.vscode/extensions.json`（由脚手架生成后移入仓库根）
- Modify: 无
- Test: 无（M0 验收 = 窗口进程启动）

**Interfaces:**
- Consumes: Task 1 的 rustc/cargo
- Produces: 可 `pnpm tauri dev` 的完整工程；`src-tauri/src/lib.rs` 的 `run()` 供后续任务注册插件/命令

- [ ] **Step 1: 生成脚手架到临时子目录**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
pnpm dlx create-tauri-app@latest scoopui --template vue-ts --manager pnpm --identifier dev.scoopui.app --tauri-version 2 --yes
ls scoopui
```

Expected: 生成 `scoopui/` 目录，含 `package.json`、`src/`、`src-tauri/`、`vite.config.ts` 等。

- [ ] **Step 2: 将脚手架内容移入仓库根（保留既有 .gitignore）**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
shopt -s dotglob nullglob
for f in scoopui/*; do
  name=$(basename "$f")
  if [ -e "$name" ]; then echo "SKIP(存在，保留): $name"; else mv "$f" .; fi
done
rmdir scoopui && echo "MOVE_DONE"
ls -a
```

Expected: 输出 `MOVE_DONE`；根目录出现 `package.json`、`src/`、`src-tauri/`、`vite.config.ts`；`.gitignore` 保留原文件（不覆盖）。

- [ ] **Step 3: 确认 Cargo 包名与窗口标题**

Read: `src-tauri/Cargo.toml`（`name = "scoopui"`）、`src-tauri/tauri.conf.json`（`identifier = "dev.scoopui.app"`、`productName`、`app.windows[0].title`）。

若 `productName` 或窗口 `title` 不是 `ScoopUI`，用编辑器改为：

```json
"productName": "scoopui",
"app": { "windows": [ { "title": "ScoopUI", "width": 1200, "height": 760, "minWidth": 960, "minHeight": 640 } ] }
```

（`productName` 保持小写以决定 exe 名 `scoopui.exe`；窗口标题用于人工识别。）

- [ ] **Step 4: 安装前端依赖**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
pnpm install 2>&1 | tail -8
```

Expected: 依赖安装完成无错误（`Done in …`）。

- [ ] **Step 5: 首次编译并启动空窗口**

Run（后台，日志落盘）:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
nohup pnpm tauri dev > "/c/Users/Administrator/AppData/Local/Temp/opencode/tauri-dev.log" 2>&1 &
echo "dev started"
```

等待首次 Rust 编译（约 1-3 分钟），然后轮询窗口：

```bash
for i in $(seq 1 60); do
  T=$(powershell -NoProfile -Command "(Get-Process scoopui -ErrorAction SilentlyContinue | Select-Object -First 1).MainWindowTitle" 2>/dev/null)
  if [ -n "$T" ]; then echo "WINDOW_TITLE=$T"; break; fi
  sleep 5
done
tail -20 "/c/Users/Administrator/AppData/Local/Temp/opencode/tauri-dev.log"
```

Expected: 输出 `WINDOW_TITLE=ScoopUI`（窗口标题非空即证明 UI 进程与 WebView 正常）。

- [ ] **Step 6: 关闭 dev 进程**

Run:

```bash
taskkill.exe //F //T //IM scoopui.exe 2>&1 | head -3
powershell -NoProfile -Command "Get-Process scoopui -ErrorAction SilentlyContinue | Measure-Object | Select-Object -ExpandProperty Count"
```

Expected: 进程数 `0`。（Vite/cargo 前台进程随后台 nohup 会话收尾，如仍有 `node` 占用端口，可 `taskkill //F //IM node.exe` 前先确认没有别的 node 项目在跑。）

- [ ] **Step 7: 提交**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add -A && git commit -m "chore: Tauri 2 + Vue3 脚手架，空窗口跑通" && git log --oneline | head -3
```

Expected: 新增一次提交。

---

### Task 3: 依赖接入（Naive UI / Pinia / 插件）与首个自动化测试

**Files:**
- Modify: `package.json`（依赖 + scripts）、`src-tauri/Cargo.toml`、`src-tauri/src/lib.rs`、`src-tauri/capabilities/default.json`、`src/main.ts`、`src/App.vue`
- Create: `src/api/tauri.ts`、`src/__tests__/tauri.spec.ts`、`vitest.config.ts`
- Test: `src/__tests__/tauri.spec.ts`、`src-tauri/src/lib.rs` 内 `#[cfg(test)]`

**Interfaces:**
- Consumes: Task 2 的脚手架工程
- Produces:
  - TS: `invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>`（`src/api/tauri.ts`，后续所有页面调用后端都走它）
  - Rust: `fn app_name() -> &'static str` 返回 `"ScoopUI"`（测试冒烟用）
  - 已注册插件：`tauri-plugin-dialog`、`tauri-plugin-opener`
  - scripts：`pnpm test`（vitest run）、`pnpm build`（vue-tsc && vite build）

- [ ] **Step 1: 写第一个后端测试（红）**

在 `src-tauri/src/lib.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_name_is_scoopui() {
        assert_eq!(app_name(), "ScoopUI");
    }
}
```

并在文件上部（`run()` 附近）暂时不提供 `app_name`，运行：

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
cargo test 2>&1 | tail -8
```

Expected: 编译失败（`cannot find function app_name`）——测试先红。

- [ ] **Step 2: 最小实现（绿）**

在 `src-tauri/src/lib.rs` 中加入：

```rust
pub fn app_name() -> &'static str {
    "ScoopUI"
}
```

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
cargo test 2>&1 | tail -6
```

Expected: `test tests::app_name_is_scoopui ... ok`，`test result: ok`。

- [ ] **Step 3: 添加 Rust 插件依赖并注册**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI/src-tauri"
cargo add tauri-plugin-dialog tauri-plugin-opener 2>&1 | tail -4
```

在 `src-tauri/src/lib.rs` 的 builder 链（脚手架默认 `.plugin(tauri_plugin_opener::init())` 位置）注册：

```rust
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
```

在 `src-tauri/capabilities/default.json` 的 `permissions` 数组中确认/加入：

```json
    "dialog:default",
    "opener:default"
```

Run:

```bash
cargo check 2>&1 | tail -4
```

Expected: `Finished`（无 error）。

- [ ] **Step 4: 写前端第一个测试（红）**

创建 `src/api/tauri.ts`：

```ts
import { invoke as tauriInvoke } from "@tauri-apps/api/core";

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(cmd, args);
}
```

创建 `src/__tests__/tauri.spec.ts`（先只写测试，暂不创建 `src/api/tauri.ts` 会红；此处实际顺序为：先建测试文件，再建实现文件，故 Step 4 只创建测试）：

```ts
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: Record<string, unknown>) => Promise.resolve({ cmd, args })),
}));

import { invoke } from "../api/tauri";

describe("invoke 包装", () => {
  it("透传命令名与参数", async () => {
    const res = await invoke<{ cmd: string; args?: Record<string, unknown> }>("list_apps", { a: 1 });
    expect(res.cmd).toBe("list_apps");
    expect(res.args).toEqual({ a: 1 });
  });
});
```

Run:

```bash
pnpm add -D vitest @vue/test-utils jsdom 2>&1 | tail -3
pnpm vitest run src/__tests__/tauri.spec.ts 2>&1 | tail -8
```

Expected: FAIL（`Cannot find module '../api/tauri'`）。

- [ ] **Step 5: 创建实现（绿）**

创建 `src/api/tauri.ts`（内容同 Step 4 所示），Run:

```bash
pnpm vitest run src/__tests__/tauri.spec.ts 2>&1 | tail -6
```

Expected: `1 passed`。

- [ ] **Step 6: 接入 Naive UI 与 Pinia，替换 App.vue 为最小中文界面**

Run:

```bash
pnpm add naive-ui pinia @tauri-apps/plugin-dialog @tauri-apps/plugin-opener 2>&1 | tail -3
```

`src/main.ts` 改为：

```ts
import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";

createApp(App).use(createPinia()).mount("#app");
```

`src/App.vue` 改为（M0 占位，M1 替换为三栏布局）：

```vue
<template>
  <n-config-provider :theme-overrides="{ common: { primaryColor: '#2080f0' } }">
    <n-message-provider>
      <div style="padding: 24px; font-family: system-ui, sans-serif">
        <h2>ScoopUI</h2>
        <p>环境就绪，进入 M1 开发。</p>
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { NConfigProvider, NMessageProvider } from "naive-ui";
</script>
```

- [ ] **Step 7: 添加 scripts 并全量验证**

`package.json` 的 `scripts` 中确保包含：

```json
    "test": "vitest run",
    "test:watch": "vitest",
    "build": "vue-tsc --noEmit && vite build"
```

Run:

```bash
pnpm test 2>&1 | tail -6
pnpm build 2>&1 | tail -6
cd src-tauri && cargo test 2>&1 | tail -4
```

Expected: vitest `1 passed`；`vite build` 完成（`built in …`）；cargo `test result: ok`。

- [ ] **Step 8: 提交**

Run:

```bash
cd "C:/Users/Administrator/Desktop/idear/scoopUI"
git add -A && git commit -m "chore: 接入 Naive UI/Pinia/对话框插件，建立首个前后端测试" && git log --oneline | head -3
```

Expected: 新增一次提交。

---

## M0 验收清单（人工）

- [ ] `rustc --version` / `cargo --version` 正常
- [ ] `cargo build` 冒烟通过（MSVC 链接器可用）
- [ ] `pnpm tauri dev` 弹出标题为 `ScoopUI` 的窗口
- [ ] `pnpm test` 与 `cargo test` 全绿
- [ ] `pnpm build` 无类型错误
- [ ] 仓库有 3 次提交（设计文档 + 脚手架 + 依赖接入）

## Self-Review 记录

- 规格覆盖：M0 行（winget 装工具链 / 脚手架 / 空窗口跑通 / git init）全部对应到 Task 1-3；git init 已在设计文档阶段完成。
- 占位符扫描：无 TBD/TODO；所有命令与代码均为可执行的具体内容。
- 类型一致性：`invoke<T>(cmd, args?)` 与 `app_name() -> &'static str` 为后续任务定义了稳定签名；`pnpm test` / `pnpm build` / `cargo test` 三个验证入口统一定义在 Task 3 Step 7。
- 已知偏差处理：create-tauri-app 模板实际文件结构以执行时 `ls` 为准；若与计划描述不同，以“移动全部内容 + 保留既有 .gitignore”为准，不改验收标准。
