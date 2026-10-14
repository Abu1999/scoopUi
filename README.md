# <img src="src-tauri/icons/128x128.png" width="42" alt="ScoopUI 图标"> ScoopUI

为 [Scoop](https://scoop.sh) 包管理器打造的 Windows 桌面图形界面。

基于 Tauri 2 + Vue 3 + Rust，覆盖 Scoop 的日常使用与运维场景：应用管理、离线全量搜索、软件源与镜像、下载加速、配置集、备份恢复、环境迁移。

## 功能特性

### 已安装应用

- 用户级与全局应用（`SCOOP_GLOBAL`）统一列表，全局应用带「全局」标记
- 可更新徽标、锁定（hold）标记；支持单应用更新/卸载/锁定
- 「全部更新」一键更新用户级与全局级可更新应用

### 商店与搜索

- 离线全量索引（解析本地软件源全部清单），输入即时出结果
- 列表 / 卡片两种视图，结果详情（版本、描述、主页、许可证）
- 安装：默认全局安装（可取消）、可选指定版本、实时日志与取消

### 软件源管理

- 添加（支持已知软件源与自定义仓库地址）、删除、更新 Scoop 与软件源
- 软件源镜像一键切换（官方 GitHub / gitee 镜像 / 201704 代理）

### 镜像与下载加速

- `URL_PROXY` 下载加速开关、连通测试与出口 IP 地理判定
- 代理（`proxy`）设置、aria2 一键安装与并发参数

### 配置集（Profile）

- 另存当前状态：软件源（含源地址）+ 应用（含全局标记）+ 下载加速设置
- 与当前状态差异对比（缺少/多余软件源与应用、源冲突、设置变化）
- 一键应用：写入设置、差集加源、安装缺失应用、可选卸载多余应用（默认关）
- 导入 `scoop export` 原生 JSON（自动识别）或导出自家格式 JSON

### 备份与恢复

- 快照内容：`config.json` + 各软件源 remote + 已安装清单 + 配置集
- 恢复配置（写回前自动备份为 `config.json.bak`）、重设软件源 remote
- 缺失应用逐条安装或「全部重装」，失败即停、可重试

### 环境迁移

- 扫描注册表中非 Scoop 安装的软件（含系统组件过滤）
- 与本地清单按置信度匹配（高：名称精确；中：可执行文件名；低：描述词重叠）
- 已在 Scoop 管理的条目自动隐藏（含全局目录）
- 引导接管：用 Scoop 安装，可选「安装前先卸载旧版」（默认关）

### 其他

- 深色模式：跟随系统 / 浅色 / 深色，重启保持
- 任务流式日志抽屉，失败给出中文建议（网络/镜像/权限/冲突）
- 安装后自动注册应用自带钩子（如 git 的右键菜单、文件关联、凭据管理器）
- 多套 Scoop 根目录探测、切换与手动添加
- 维护操作：清理缓存、清理旧版本（用户/全局）、checkup、重置 shims

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + Vite + Naive UI + Pinia |
| 后端 | Rust（全部 IO / 进程 / 网络 / 注册表） |
| 测试 | Rust 单元/集成测试（cargo test）、Vitest（前端） |

## 环境要求

- Windows 10 及以上（x64）
- 已安装 [Scoop](https://scoop.sh)（本应用管理的目标）
- [Rust 工具链](https://rustup.rs)（rustup + MSVC Build Tools）
- Node.js 20+ 与 pnpm
- WebView2（Windows 10 通常已内置）

## 快速开始

```bash
# 安装前端依赖
pnpm install

# 开发模式（自动打开应用窗口）
pnpm tauri dev

# 构建发行版
pnpm tauri build
```

## 测试

```bash
# Rust 单元与集成测试
cd src-tauri && cargo test

# 前端测试
pnpm test

# 前端类型检查与构建
pnpm build

# 真实环境流程测试（需本机有可用 scoop 环境；会真实卸载/重装 7zip 等）
cd src-tauri && cargo test --test live_flows -- --ignored --nocapture
```

## 首次使用（自动初始化 Scoop）

安装包内置 Scoop 与 main 软件源快照（浅克隆，含 `.git`）。首次启动若未检测到 Scoop，会显示引导页：

- 「一键初始化（内置，无需下载）」：离线释放内置快照到目标目录（默认 `%USERPROFILE%\scoop`，可修改），可选创建 `scoop` 命令并加入 PATH
- 「在线安装最新版」：适合网络畅通、希望直接安装最新 Scoop 的场景
- 「已经装过 Scoop？选择根目录」：接入已有安装

初始化后即可使用全部功能（软件源索引、安装/卸载/更新应用等）。装好 git 后可用「更新 Scoop 与软件源」获取最新内容；无 git 时安装应用不受影响。

内置快照随 ScoopUI 版本刷新（`scripts/build-scoop-bundle.ps1` 重新打包），引导页会显示快照版本。

**环境清理**：由 ScoopUI 初始化的 Scoop 环境会写入标记；卸载本应用时安装程序会询问是否连同删除该目录（含已安装应用）；也可随时在「设置 → 运行环境」下方点击「删除该环境」（同时清理 PATH 条目）。手动安装的 Scoop 不会被触碰。

## 数据目录

| 路径 | 用途 |
|---|---|
| `%APPDATA%\scoopui\settings.json` | 根目录、主题等设置 |
| `%APPDATA%\scoopui\profiles\` | 配置集文件（`<名称>.json`） |
| `%APPDATA%\scoopui\snapshots\<时间戳>\` | 备份快照 |
| `%LOCALAPPDATA%\scoopui\icons\` | 图标缓存 |

## 项目结构

```
src/                       前端（Vue 3）
  api/                     命令封装与类型
  components/              布局与业务组件
  pages/                   页面（已安装/商店/软件源/维护/配置集/备份/迁移/设置）
  stores/                  Pinia 状态（根目录/应用/包/配置集/快照/迁移/外观等）
  utils/                   任务框架、错误建议、安装钩子等
src-tauri/                 后端（Rust）
  src/commands/            Tauri 命令层（薄适配）
  src/domain/              领域逻辑（清单、索引、根目录、配置、镜像、配置集、快照、注册表、迁移等）
  tests/                   真实环境集成测试
docs/
  acceptance.md            手工验收清单（M1-M7）
  superpowers/specs/       设计文档
  superpowers/plans/       实现计划
```

## 常见问题

**更新 git（或其他正在使用的应用）失败**

Scoop 更新应用前会检测是否有相关进程在运行。若通过本应用的终端、VS Code 等正在使用 git，会提示
`The following instances of ... are still running`。请关闭相关窗口后重试，或在独立的管理员 PowerShell 中执行：

```powershell
scoop update git -g
```

**提示「Uncommitted changes detected. Update aborted.」**

你使用的 Scoop 是国内增强版（本地包含 URL_PROXY 等补丁），Scoop 自身的自动更新会因工作区改动被跳过；软件源与应用更新不受影响。

**全局安装失败**

全局应用需要管理员权限，请以管理员身份运行本应用（或使用 `scoop` 命令配合管理员终端）。

**环境迁移扫描不到某软件**

仅扫描注册表 `Uninstall` 键中登记的软件；命令行工具（如 Android platform-tools）通常没有卸载条目，不会出现在列表中。

## 非目标

- 不自动提权（UAC 触发时仅提示）
- 不代搬旧软件的用户数据（提供引导）
- 无托盘常驻与开机自启（后续可评估）
- 暂不提供多语言（界面为中文）
