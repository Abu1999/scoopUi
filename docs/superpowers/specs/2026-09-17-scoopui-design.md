# ScoopUI 设计文档

日期：2026-09-17
状态：已确认（用户批准）
目标平台：Windows 10+（x64）

## 1. 背景与目标

为 Scoop 包管理器提供一个桌面图形界面，覆盖日常使用与运维场景：

- 应用管理：安装 / 卸载 / 更新 / 锁定（hold/unhold）
- 浏览与搜索：离线全量搜索、包详情（版本、描述、主页、依赖）
- 桶管理：添加 / 删除 / 刷新
- 维护：缓存清理、旧版本清理、checkup、reset
- 国内源：镜像源切换、下载加速（URL_PROXY / proxy / aria2）
- 本地环境：注册表扫描已装软件，匹配并迁移到 scoop 管理
- 多套 scoop 根目录：检测、切换、独立操作
- 配置集（Profile）：应用集合差异化安装与切换
- 备份 / 恢复：配置与清单快照

非目标（本期不做）：

- 自动提权（UAC 触发时仅提示）
- 自动迁移旧软件的用户数据（提供引导，不代搬）
- 托盘常驻/开机自启
- 国际化（先中文，字符串按可 i18n 结构组织）

## 2. 技术栈与架构

```
Vue 3 + TypeScript + Vite + Naive UI + Pinia   （前端：展示与交互）
        │  tauri invoke（命令） ↑↓ tauri event（日志流）
Rust（Tauri 2 后端）：全部 IO / 进程 / 网络 / 注册表
        ├─ 文件系统：apps、buckets、manifest.json、install.json、config.json
        ├─ 进程：powershell → 目标根 scoop.ps1、git、卸载程序
        ├─ 网络：favicon 抓取（直连 → URL_PROXY → favicon.im）
        └─ 注册表：Uninstall 键扫描
```

- UI 库：Naive UI（中文文档、主题定制、组件齐全）
- 应用数据目录：
  - 设置 / 配置集 / 快照：`%APPDATA%\scoopui\`
  - 图标缓存：`%LOCALAPPDATA%\scoopui\icons\`
- 所有 scoop 命令以流式方式执行，stdout/stderr 实时推送到前端日志抽屉，支持取消（杀进程树）
- 任务串行队列（scoop 自身有缓存锁，避免并发冲突）

## 3. 关键实现事实（已实测验证）

| # | 事实 | 证据 |
|---|---|---|
| 1 | 多根调用方式：必须直接调用目标根的 `<root>\apps\scoop\current\bin\scoop.ps1`，shim 内是写死的绝对路径不可复用；同时注入 `SCOOP=<root>` | `~/scoop/shims/scoop.cmd` 内容；`core.ps1:1496` |
| 2 | 根目录解析优先级：`SCOOP` 环境变量 → config `root_path` → ps1 自定位（`$PSScriptRoot\..\..\..\..`）→ `~/scoop` | `core.ps1:1496` |
| 3 | 全局目录：`SCOOP_GLOBAL` → config `global_path` → `%ProgramData%\scoop`；缓存目录：`SCOOP_CACHE` → config `cache_path` → `<root>\cache` | `core.ps1:1498-1506` |
| 4 | 配置文件位置：普通模式 `%USERPROFILE%\.config\scoop\config.json`；**portable 模式 `<root>\config.json` 优先**（若存在） | `core.ps1:1468-1489` |
| 5 | 已装应用数据源：`apps/<app>/current/manifest.json` + `install.json`（含 bucket、architecture、hold、updated） | 本机实测 + `manifest.ps1:95`、`scoop-hold.ps1:66` |
| 6 | 本地清单索引：4955 个 JSON 全量解析 0.56s（Python），Rust 更快；无 BOM 问题 | 实测 |
| 7 | 更新检测：已装 manifest 版本 vs 桶内 manifest 版本本地对比即可 | 实测（git 2.53.0.2 vs 桶内 2.55.0.5） |
| 8 | 清单几乎没有 icon 字段（0/4017）→ 图标必须走 favicon 方案 | 实测统计 |
| 9 | 用户使用 gitee 增强 fork，内置 `URL_PROXY` 下载加速（按出口 IP 地理判定直连/代理，失败时一律走代理）；配置键 `url_proxy`，默认 `https://scoop.201704.xyz` | `core.ps1:49-87` |
| 10 | Scoop 支持 `proxy`、`aria2-enabled`、`aria2-split`、`aria2-max-connection-per-server` 等下载配置 | `download.ps1:367-380` |
| 11 | config 键写入时均小写（`get_config`/`set_config` 会 `ToLowerInvariant`） | `core.ps1:150-165` |
| 12 | 本机环境：Win10 22H2（19045）、WebView2 149 已装、winget 可用、Node 24 + pnpm 11、PS 5.1、C 盘余 62GB；注册表 Uninstall 共 97 条 | 实测 |
| 13 | 网络实测：npmjs 0.38s、npmmirror 0.22s、crates.io 0.63s、rsproxy 0.2s、rustup 2.9s、github 直连 1.28s、favicon.ico 直连 0.6-1.9s | 实测 |

## 4. 核心机制设计

### 4.1 多根目录管理

- 探测：`%SCOOP%`、config `root_path`、`~/scoop`、`%SCOOP_GLOBAL%`，加手动添加路径
- 校验：目录必须包含 `apps/`、`buckets/`、`shims/`
- 切换 = 仅改 app 内 `active_root`；每条命令：
  - 可执行文件：`<root>\apps\scoop\current\bin\scoop.ps1`
  - 环境注入：`SCOOP=<root>`（最高优先级，保证确定性）
- 不动系统环境变量；提供显式"设为系统默认"按钮（写用户级环境变量）
- 全局应用（`SCOOP_GLOBAL`）不随根切换，UI 明确标注
- 切换根后重新解析：生效的 configFile（portable 优先）、桶列表、已装应用

### 4.2 命令执行框架

- `powershell -NoProfile -ExecutionPolicy Bypass -Command "[Console]::OutputEncoding=[Text.Encoding]::UTF8; ... & '<root>\apps\scoop\current\bin\scoop.ps1' <args>"`
- 流式读取 stdout/stderr（逐行）→ Tauri event `scoop-log` → 前端日志抽屉
- 支持取消：Windows Job Object 或 taskkill /T 杀进程树
- 错误分类映射（中文提示 + 建议动作）：
  - 网络超时 → 检查网络或切换 URL_PROXY
  - 哈希校验失败 → 镜像未同步，建议 `scoop update <app>` 或清缓存
  - 权限不足 → 提示以管理员运行或改用全局安装
  - manifest 冲突 → 提示先卸载同名应用

### 4.3 本地数据源

- 已装列表：遍历 `<root>\apps\*`，读 `current` 指向版本的 `manifest.json` + `install.json`
- 搜索索引：启动时并行解析 `<root>\buckets\*\bucket\**\*.json`（含 deprecated 子目录）→ 内存索引 + 磁盘缓存（`%APPDATA%\scoopui\index\<root-hash>.json`），桶更新后失效重建
- 更新检测：本地版本对比（可更新徽标），可选联网 `scoop status` 复核

### 4.4 图标管线

优先级链：清单 `icon` 字段（几乎不存在）→ `homepage` 域名 favicon → 首字母彩色头像

- favicon 抓取：先解析 homepage HTML 的 `<link rel="icon">`，再退到 `/favicon.ico`；GitHub 系域名可套 `url_proxy` 前缀
- 懒加载：仅抓可见项，并发上限 6；磁盘缓存 + 负缓存（失败 7 天内不重试）
- 缓存键：`sha1(icon_source_url)`，存 `%LOCALAPPDATA%\scoopui\icons\`
- 失败静默降级为首字母头像（名字哈希决定颜色）
- 设置项：图标代理前缀（默认读取当前 `url_proxy`）、缓存清理按钮

### 4.5 镜像源与加速

- 镜像预设：
  - scoop 仓库：官方 / gitee（`https://gitee.com/scoop-installer/scoop`）/ 201704 代理
  - 桶 remote：官方 / gitee 镜像 / `https://scoop.201704.xyz/https://<源>` 代理
  - 应用方式：`git -C <repo> remote set-url origin <url>`；scoop 自身仓库同理（改 `scoop_repo` 配置 + 对应 git remote）
- `URL_PROXY`：fork 独有下载加速开关，UI 提供设置 + 连通测试 + 当前出口 IP 判定展示
- `proxy`：`scoop config proxy <host:port>`（供 aria2/请求使用）
- aria2：一键安装 aria2 + `scoop config aria2-enabled true`；暴露 split / max-connection-per-server
- 特性检测：读取 config 实际存在的键，fork 独有项（url_proxy、root_path、global_path、cache_path）单独成组展示

### 4.6 配置集（Profile）

```json
{
  "name": "工作",
  "buckets": ["main", "extras", "java"],
  "apps": [{"name": "git", "bucket": "main"}, {"name": "python", "bucket": "main"}],
  "settings": {"url_proxy": "https://scoop.201704.xyz"}
}
```

- 应用 = 加桶（差集）+ 安装缺失应用（可选卸载多余，默认不卸）
- 另存当前状态为配置集；与当前状态差异对比视图
- 存储：`%APPDATA%\scoopui\profiles\*.json`；兼容 `scoop export` 原生 JSON 导入

### 4.7 备份 / 恢复

快照内容：`config.json` + 各桶 remote 列表 + 已装清单（name/bucket/version/hold）+ 配置集
存储：`%APPDATA%\scoopui\snapshots\<timestamp>\`
恢复：写回 config.json、重设桶 remote、列出需重装应用并引导执行

### 4.8 环境扫描与迁移

- 注册表枚举：`HKLM\...\Uninstall`、`HKLM\...\WOW6432Node\...\Uninstall`、`HKCU\...\Uninstall`（winreg crate）
- 过滤：SystemComponent != 1、无 ParentKeyName、DisplayName 非空
- 匹配评分（与本地索引）：
  1. 规范化名精确匹配（小写、去空格/标点）→ 高置信
  2. manifest `bin` 可执行名 与 InstallLocation 内 exe 名匹配 → 中
  3. 描述词重叠度 > 阈值 → 低
- 展示：置信度分组；每行操作 = 搜索指定包 / 用 scoop 安装（可选先跑旧版 UninstallString）
- 已在 scoop 管理的条目（路径位于任何已知根下）标记跳过

## 5. 页面结构（三栏主框架）

侧边栏：**已安装 / 商店 / 桶管理 / 维护 / 环境迁移 / 配置集 / 备份 / 设置**

- 已安装：左列表（图标 + 名称 + 版本 + 可更新徽标 + hold 标记）→ 右详情（大图标、描述、主页、依赖、persist 目录、安装目录、操作按钮）
- 商店：搜索框 + 结果列表（可切卡片网格）+ 详情 + 安装（选桶/版本/全局）
- 全局：顶部根目录切换器、任务日志抽屉（可展开/取消）、危险操作确认弹窗

## 6. 错误处理

- 命令失败：退出码 + stderr 尾部 → 按分类映射中文提示与建议动作
- 根目录无效/缺少 scoop：引导修复或移除
- config.json 损坏：备份为 `.bak` 后重建默认
- 图标抓取失败：静默降级，负缓存
- 写配置前：解析实际生效 configFile（portable 优先），避免写错位置
- MVP 不自动提权；检测到权限错误给出提示

## 7. 测试策略

- Rust 单元测试：manifest 解析、版本比较、install.json 读取、注册表匹配评分、config 读写（portable/普通）、URL 改写、图标缓存键、错误分类
- Rust 集成测试：`src-tauri/tests/fixtures/` 假 scoop 根目录（apps/buckets/manifest 样本），不依赖真实 scoop
- 前端 Vitest：Pinia stores + 关键组件（mock invoke）
- 每个阶段附手工验收清单；全程 TDD

## 8. 实施阶段

| 阶段 | 内容 | 验收 |
|---|---|---|
| M0 | winget 装 Rustup + VS BuildTools；Tauri 脚手架；空窗口跑通；git init | `pnpm tauri dev` 出窗口 |
| M1 | 根目录探测/切换、三栏布局、已装列表+详情、桶列表、命令框架+日志抽屉 | 能列出本机 9 个应用并实时跑日志命令 |
| M2 | 清单索引、商店搜索、图标管线 | 搜 git 秒出；图标可见（favicon/首字母） |
| M3 | 装/卸/更新/hold、桶管理、维护清理 | 真实安装/卸载一个小应用 |
| M4 | 镜像源/URL_PROXY/aria2/更新检测 | 可切换源并生效；显示可更新徽标 |
| M5 | 配置集、备份/恢复 | 导出/应用配置集；快照恢复 config |
| M6 | 环境扫描与迁移 | 扫出 97+ 条目并完成一次匹配接管 |
| M7 | 打磨 | 错误提示、深色模式、空态、验收清单通过 |

## 9. 风险与注意

- gitee fork 与官方 Scoop 存在行为差异（URL_PROXY/ROOT_PATH 等键）：全部走特性检测，不硬编码假设
- `URL_PROXY` 依赖 `ip.glimmer.ltd` 服务，不可达时 scoop 自身会一律走代理（可能反而变慢）：UI 显示连通测试
- 图标质量参差（favicon 尺寸不一），详情页统一 64px 容器，列表 20px
- 卸载/安装长任务可能弹 UAC 或安装器窗口：日志抽屉显示完整输出，取消后提示可能残留半装状态
- 多根下 SCOOP_GLOBAL 共享：跨根全局应用列表需要合并展示并标注来源
