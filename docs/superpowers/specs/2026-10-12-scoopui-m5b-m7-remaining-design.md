# ScoopUI 剩余里程碑设计（术语替换 + M5b + M6 + M7）

日期：2026-10-12
状态：已确认（用户批准）
上游：`docs/superpowers/specs/2026-09-17-scoopui-design.md` §4.7（备份/恢复）、§4.8（环境扫描与迁移）、§8（M7 打磨）
里程碑：术语替换（先行）、M5b 备份/恢复、M6 环境迁移、M7 打磨

## 0. 已确认决策（用户问答结论）

1. 「桶」（Scoop bucket）在用户可见处统一替换为「**软件源**」；代码标识符与命令名（`bucket_add`、`BucketInfo` 等）不变。
2. 剩余功能全做：M5b 备份/恢复 + M6 环境迁移 + M7 打磨。
3. 快照恢复采用**半自动**：写回 config.json（先自动备份当前为 `.bak`）+ 重设各软件源 remote + 列出缺失应用供用户逐条重装（复用任务框架）。
4. 环境迁移提供**可选代卸载**（默认关）：勾选后先跑注册表 UninstallString，失败不阻断安装。
5. 深色模式偏好持久化到后端 `%APPDATA%\scoopui\settings.json`（与根目录设置同源），设置页三态切换（跟随系统/浅色/深色）。
6. 执行组织：术语替换 → M5b → M6 → M7 共 4 份计划依次执行，全程子代理驱动、逐任务评审、最终全分支评审，无中途停顿。

## 1. 术语替换（桶 → 软件源）

### 1.1 替换映射

| 原文 | 新文案 |
|---|---|
| 桶管理 | 软件源管理 |
| 添加桶 / 删除桶 | 添加软件源 / 删除软件源 |
| 更新 Scoop 与桶 | 更新 Scoop 与软件源 |
| 桶名称 / 桶名 | 软件源名称 |
| 已知桶（列表） | 已知软件源（列表） |
| 桶不存在 / 桶名称非法 | 软件源不存在 / 软件源名称非法 |
| 桶不是 git 仓库，无法切换源 | 软件源不是 git 仓库，无法切换源 |
| 桶 {{name}} 已切换 | 软件源 {{name}} 已切换 |
| 来自 {{bucket}} 桶 | 来自软件源 {{bucket}} |
| 缺少桶 / 多余桶（不处理） / 桶（n） | 缺少软件源 / 多余软件源（不处理） / 软件源（n） |
| 未知桶 | 未知软件源 |
| 应用 / 桶数量 | 应用 / 软件源数量 |
| 添加桶 {{name}}（应用步骤标签） | 添加软件源 {{name}} |

### 1.2 范围

- 前端：`SideBar.vue`、`BucketsPage.vue`、`MirrorSection.vue`、`PackageDetail.vue`、`ProfilesPage.vue`、`SettingsPage.vue`、`stores/profiles.ts`，以及全部相关测试断言与用例名
- 后端：`commands/buckets.rs`、`commands/mirrors.rs`、`domain/scoopops.rs` 的中文用户可见消息，及 `mirrors.rs` 内既有测试文案
- 不改：命令名、类型名、变量名、`docs/` 历史文档
- 验收：`grep -r 桶 src src-tauri/src` 为 0

## 2. M5b 备份/恢复

### 2.1 快照存储

- 目录：`%APPDATA%\scoopui\snapshots\<epoch秒>\`（`paths::snapshots_dir(appdata)`）
- `meta.json`：`{ "version": 1, "created": u64, "root": string, "config_path": string, "config_present": bool, "app_count": usize, "bucket_count": usize, "profile_count": usize }`
- `config.json`：快照时生效配置的副本（仅 `config_present` 为 true 时存在）
- `buckets.json`：`[{ "name", "source": string|null }]`
- `apps.json`：`[{ "name", "bucket": string|null, "version", "hold": bool, "global": bool }]`
- `profiles/`：配置集目录内 `.json` 的副本（目录不存在则跳过）

### 2.2 命令（`domain/snapshots.rs` + `commands/snapshots.rs`）

| 命令 | 返回 | 行为 |
|---|---|---|
| `list_snapshots` | `Vec<SnapshotSummary>`（id/created/root/计数） | 读目录；损坏/不可解析的快照跳过；按 created 倒序 |
| `create_snapshot` | `SnapshotSummary` | 扫描当前激活根：`buckets::scan`、`scan_installed`、`configfile::resolve/load`、复制 profiles 目录 |
| `delete_snapshot(id)` | `()` | 删除整个快照目录 |
| `restore_snapshot_plan(id)` | `RestorePlan` | 只读计算：目标 config 路径、`config_present`、需重设源的源（本地存在且为 git）、缺失源（快照有而本地无；含 source）、缺失应用（快照有而本地无，含 global）、`root_matches` |
| `restore_snapshot_config(id)` | `()` | 写回配置到当前激活根的生效 config 路径；写前复制当前文件为 `config.json.bak`；tmp+rename 原子写；快照无 config 时报「该快照未包含配置文件」 |
| `restore_snapshot_buckets(id)` | `{ reset: Vec<String>, missing: Vec<BucketRef>, skipped: Vec<String> }` | 对快照中每个有远程 source 的桶：本地存在且为 git → `git remote set-url origin`；本地缺失 → 归入 missing；source 为本地路径/缺失 → skipped |

`RestorePlan`：`{ config_path, config_present, reset_buckets: Vec<BucketRef>, missing_buckets: Vec<BucketRef>, missing_apps: Vec<SnapshotApp>, root: string, root_matches: bool }`。

### 2.3 恢复流程（前端编排）

1. 「恢复配置与软件源」按钮：`restore_snapshot_config` → `restore_snapshot_buckets`（同步命令，无流式步骤）
2. 缺失软件源（有 source）：逐条 `bucket_add(name, source, taskId)`（runTask，失败即停）
3. 缺失应用：列表逐条「安装」按钮 + 「全部重装」（逐条串行、失败即停、剩余待执行；复用 M5a 模式）
4. 全部完成后刷新 apps/buckets 并重算 plan；`root_matches=false` 时页面顶部警示「快照来自 <root>，当前根为 <当前>」，不阻断

### 2.4 备份页（`BackupPage.vue` + `stores/snapshots.ts`）

- 顶部「创建快照」按钮 + 说明文字
- 列表：时间（本地化）、根路径、`应用 n · 软件源 n · 配置集 n`；选中显示详情
- 详情：操作（恢复配置与软件源 / 删除）+ 恢复计划区块（缺失应用/缺失源/源重设清单/root 警示）+ 缺失应用安装清单
- 空态：「还没有快照，点击「创建快照」保存当前状态」

## 3. M6 环境迁移

### 3.1 注册表扫描（`domain/registry.rs`）

- 通过 windows-sys（既有依赖，新增 feature `Win32_System_Registry`）枚举：`HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall`、`HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall`、`HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
- 读取字段：`DisplayName`、`DisplayVersion`、`Publisher`、`InstallLocation`、`UninstallString`、`SystemComponent`、`ParentKeyName`
- 过滤：`SystemComponent == 1` 跳过；存在 `ParentKeyName` 跳过；`DisplayName` 为空跳过
- `RegistryApp { key_path, name, version, publisher, install_location, uninstall_string, managed }`；`managed = install_location 位于任一已知根（settings.known_roots + 当前根）下`
- 新命令 `scan_installed_registry() -> Vec<RegistryApp>`

### 3.2 匹配（`domain/migrate.rs`）

评分（对当前活动根的索引）：
1. **高**：规范化名（小写、去空格与标点）与包名精确相等
2. **中**：manifest `bin` 可执行名（去 `.exe` 后缀、小写）与 `InstallLocation` 内一级目录 exe 名有交集
3. **低**：`DisplayName` 与包 `description` 的词重叠超阈值（≥ 2 个共享词）

- 索引扩展：`PackageEntry` 增加 `bin: Vec<String>`；索引缓存版本号升级（旧缓存自动重建）
- `match_apps(apps, entries) -> Vec<MigrationEntry>`：`MigrationEntry { app: RegistryApp, matches: Vec<MigrationMatch> }`；`MigrationMatch { name, bucket, confidence: "high"|"medium"|"low", version, description }`
- 新命令 `match_registry_apps() -> Vec<MigrationEntry>`（内部：扫描 + index）

### 3.3 代卸载（`run_registry_uninstall`）

- 命令：`run_registry_uninstall(entry_key: String, task_id: String)`：按 `entry_key` 重新打开注册表键并读取 `UninstallString`（不接受前端任意命令串；键必须匹配三处 Uninstall 根之一），以 `cmd /C <uninstallstring>` 方式流式执行（复用 `runner::spawn_task` 机制），可取消
- 失败不阻断：前端在调用后无论结果都继续安装

### 3.4 迁移页（`MigratePage.vue` + `stores/migrate.ts`）

- 顶部「扫描本机已安装软件」+ 统计（共 n 条 · 已由 scoop 管理 m 条）+「显示已管理条目」开关
- 分组：高置信 / 中置信 / 低置信 / 未匹配；每行：名称、版本、发行商、安装目录；匹配包以标签展示 `bucket/name`（可多选？默认取第一个高/中匹配）
- 操作：每行「用 scoop 安装」（当有匹配时；含「安装前先卸载旧版」勾选，默认关）与「查看」（跳转商店搜索）
- 「查看」实现：`ui` store 增加 `storeSearchQuery: string | null`，迁移页写入后 `setPage("store")`；`StorePage` 挂载时若有值则先执行搜索并清空
- 安装：`runTask` →（勾选时先 `run_registry_uninstall`，失败提示但继续）→ `installPackage(name, bucket, null, false, taskId)`
- 空态：「点击「扫描本机已安装软件」开始」；扫描中 loading

## 4. M7 打磨

### 4.1 深色模式

- `settings.rs` 的 `Settings` 增加 `theme: Option<String>`（`"system"|"light"|"dark"`，默认 `system`，未知值按 system 处理）
- 新命令：`get_app_settings() -> { theme }`、`set_theme(theme: String)`（校验取值）
- 前端：`stores/appearance.ts`（加载偏好、`matchMedia("(prefers-color-scheme: dark)")` 监听、计算 effectiveTheme）；`App.vue` 的 `n-config-provider` 传 `:theme="effectiveTheme === 'dark' ? darkTheme : null"`
- 设置页「外观」区块：`n-radio-group` 三态（跟随系统/浅色/深色）
- 样式适配：`AppShell/SideBar/TopBar/LogDrawer` 与各页硬编码色（`#efeff5`、`#f5f7fa`、`#fff`、`#eaf3ff` 等）改为随主题的 CSS 变量（`:root` 与 `.dark` 两套），保证暗色下可读

### 4.2 空态补齐

- 商店未搜索/无结果、软件源列表为空、迁移未扫描、备份无快照、配置集已做；统一「标题 + 一句引导 + 可选操作按钮」样式

### 4.3 错误提示分类（前端 `utils/errorHint.ts`）

- 输入：任务的 stderr 行集合；输出：建议文案（无匹配返回 null）
- 映射：网络超时/连接失败 → 「检查网络或切换下载加速代理」；哈希校验失败 → 「镜像未同步：运行 scoop update 或清理缓存后重试」；权限不足/拒绝访问 → 「权限不足：以管理员身份运行或改用全局安装」；manifest 冲突/同名已存在 → 「先卸载同名应用再试」
- 接入：`utils/tasks.ts` 的 `runTask` 返回的 `TaskRecord` 增加可选字段 `hint?: string | null`（从 logs store 该 taskId 的 stderr 行计算；可选以保证既有构造点不需同步修改）；各页失败提示统一为 `操作失败（退出码 X）：{hint}`（无 hint 时保持原文案）

### 4.4 验收清单

- 新增 `docs/acceptance.md`：覆盖 M1-M7 的中文手工验收步骤（安装/卸载/更新/hold、搜索/图标、镜像/加速、配置集、备份恢复、环境迁移、深色模式与错误提示）

## 5. 测试与验证策略

- 每个里程碑内联单测：Rust（domain 层为主）+ Vitest（store 与页面）
- 术语替换：`grep -r 桶 src src-tauri/src` 为 0；测试断言同步
- M5b：快照创建/读取/删除/计划计算（tempdir）、config 写回与 `.bak`、桶 remote 重设（tempdir git 仓库）、恢复编排 store 测试与页面测试
- M6：注册表解析过滤（可注入的读取函数 + fixture 数据，不依赖真实注册表）、匹配评分各分级、索引 bin 扩展、页面测试
- M7：settings 主题读写、appearance store（system 监听）、errorHint 映射、深色模式渲染冒烟、验收清单存在性
- 每阶段门禁：`cargo test` 全绿、`pnpm test` 全绿、`pnpm build`（vue-tsc）无错

## 6. 非目标与已知取舍

- 快照不做导出/导入文件、不做自动轮转与自动快照（应用配置集前不自动快照）
- 恢复不自动添加缺失软件源以外的应用卸载、不回滚已安装应用
- 迁移不做用户数据搬迁（沿用主设计非目标）、不做提权自动化
- 代卸载依赖 UninstallString 本身可用（MSI/UAC 交互由系统接管，失败不阻断安装）
- 深色模式仅覆盖应用自身 UI，不改系统设置
