# AGENTS.md

## 项目概览

Windows 开机自启动命令行管理器。

## 关键命令

```bash
npm run tauri:dev     # 启动开发（Vite + Rust 编译 + GUI 窗口，长时间运行）
npm run tauri:build   # 生产打包 → NSIS 安装器
npm run build         # 仅前端构建（tsc + vite）
npm run lint          # oxlint
cd src-tauri && cargo test    # Rust 单元测试（32 个）
cd src-tauri && cargo check   # Rust 类型检查
```

**注意**：`cargo test` 仅测试 lib crate 的单元测试；main.rs 没有测试。


## 技术栈

**Tauri v2 + React 19 + SQLite**

## 架构

### 后端模块 (src-tauri/src/)

| 文件 | 职责 | 注意 |
|---|---|---|
| `storage.rs` | SQLite CRUD、`Command`/`CommandStep`/`Run` 结构、DB 迁移、runs 去重事务 | `Command.enabled` 控制是否允许启动；`Mutex<Connection>` 满足 Tauri `State: Send+Sync`；GUI/CLI 共享 DB |
| `commands.rs` | Tauri 命令薄层，执行类命令委托 `runner.rs` | `set_command_enabled`；保持精简 |
| `runner.rs` | 统一执行入口：去重、spawn、PID 解析、状态轮询、停止 | GUI/CLI/自启都走这里 |
| `process.rs` | PID 存活/复用校验、`taskkill /T /F`、WT shell PID 发现 | 使用 `windows`/`Wdk` feature；NT 命令行读取 |
| `script.rs` | 终端参数构造、marker 注入、spawn（返回 PID）、WT tabs | `build_*` 纯函数有单测 |
| `terminal.rs` | 终端检测、WT settings.json 解析、bash 路径查找 | 纯检测逻辑，无副作用 |
| `main.rs` | CLI 入口（`openstart --cli`）、单实例互斥量、CLI 控制台挂接 | release 是 GUI 子系统，必须 attach 父控制台 |
| `lib.rs` | Tauri 应用启动、托盘、自启执行、invoke_handler 注册 | 关闭窗口 → 隐藏到托盘，不退出 |
| `autostart.rs` | HKCU Run 注册表操作 | Windows only |
| `settings.rs` | JSON 配置文件读写（`app_autostart`、`startup_delay_seconds`） | 数据目录：`%APPDATA%\openstart\` |

### 前端 (src/)

| 文件 | 职责 | 注意 |
|---|---|---|
| `App.tsx` | 页面骨架、数据加载、runs 轮询、分组渲染、CRUD 动作 | **保持精简**，展示逻辑下沉到组件 |
| `CommandRow.tsx` | 命令行：启用开关、运行状态/PID、步骤展开、终端标签、运行/编辑/删除 | 单行密度优先 |
| `CommandEditor.tsx` | 新建/编辑弹窗：步骤拖拽、终端/分组选择、开机自启开关 | 保存时 `command = steps[0].cmd` 双写 |
| `RunsPanel.tsx` | 顶部 Running 条：PID、时长、单条/全部停止 | 无活动 run 时自动隐藏 |
| `SettingsPage.tsx` | 设置页、执行日志弹窗、更新检查 | 设置在该页内加载并保存 |
| `ui.tsx` | 通用组件：`Switch` / `IconButton` / `Modal` / `ConfirmDialog` | 样式类统一放 App.css |
| `icons.tsx` | SVG 图标集（Lucide 24px 线性图标） | 不要用 emoji 当图标 |
| `format.ts` | 时间/时长格式化（`fmtDateTime`、`fmtRelative`、`fmtElapsed`） | 纯函数，无依赖 |
| `terminals.ts` | 终端 id → 显示名 / 颜色点 | WT profile 显示为 `WT: <profile>` |
| `index.css` | 设计 token（`--bg-*`、`--text-*`、`--border-*`、圆角、字体）+ 全局基础样式 | **调整配色优先改这里** |
| `App.css` | 组件样式（导航、分组、命令行、弹窗、设置、Toast） | 单一强调色 + 三级文字 + 发丝分隔线，避免卡片套卡片 |
| `api.ts` | `invoke()` 包装，类型化调用 Tauri 命令 | 新增命令需两端同步 |
| `types.ts` | TypeScript 接口（`Command`、`Run`、`AppSettings` 等） | 必须与 Rust 结构体字段一致 |

### 执行路径（统一入口）

GUI 单条 / GUI 分组 / CLI / 开机自启 → `runner.rs` → `storage.begin_runs()`（事务内刷新存活 + 去重 + 插入 `starting`）→ `script::spawn_*`（直接终端返回 PID；WT 注入 marker）→ 更新 `runs` 表。

- 同命令/同分组成员/同 adhoc 命令并发时只会启动一次；冲突返回 `RunError { code: "already_running", run_id, pid }`。
- **禁用命令不参与任何启动路径**：单条返回 `disabled`，分组跳过禁用成员（全禁用返回 `no_enabled_commands`），开机自启/`run-all` 只取 `auto_start=1 AND enabled=1`。GUI 用行内开关控制启用/禁用（禁用行整行降级显示）。
- GUI 每 2.5s 轮询 `list_runs` 刷新 PID/存活状态；CLI 用 `status`。
- `stop` 用 `taskkill /PID <pid> /T /F` 结束整个进程树。
- WT 的 `wt.exe` PID 只是 launcher；通过 `OSRUN-<run-id>` marker + `NtQueryInformationProcess(ProcessCommandLineInformation)` 找到真实 shell PID（优先父进程为 WindowsTerminal/OpenConsole/conhost）。
- `keep_terminal_open=true` 时，命令跑完窗口未关仍算 `running`，直到关窗或 Stop；这是去重语义的一部分。

### CLI 子命令

```
list | add | delete | run | group | run-all | exec
status [<run-id>] [--all]
stop <run-id> | --command <id> | --group <name> | --all
enable | disable | help
```

`run/group/exec` 支持 `--force`（先停已有实例再启动）。退出码：0 成功、1 错误、2 already running。

## 多步命令 (steps) 机制

- 数据：`Command.steps: Vec<CommandStep>`，存为 JSON 到 `steps TEXT` 列
- 解析：`steps_for_command()` → 空 steps 则用旧 `command` 字段兼容
- 脚本拼接：`build_chained_script(steps, shell)` 按 shell 类型拼链式命令

### 分隔符规则

| Shell 类型 | 延迟分隔符 | 零延迟分隔符 |
|---|---|---|
| `powershell` | `; Start-Sleep -Seconds N; ` | `; ` |
| `cmd` | ` & timeout /t N /nobreak >nul & ` | ` & ` |
| `gitbash` | ` && sleep N && ` | ` && ` |


### 保存时双写

`command` 字段 = `steps[0].cmd`，保 CLI `list` 和旧代码兼容。

## 终端相关

### 终端 ID 体系

- `powershell` / `cmd` / `gitbash` — 独立启动（不经过 wt.exe）
- `terminal` — Windows Terminal 默认 profile（PowerShell）
- `terminal:ProfileName` — WT 指定 profile（名称从 settings.json 动态读取）


## 发布

发布版本完全由 git tag 决定。

### 分支
```
dev     ← 日常开发，所有 commit 先到这里
main    ← 主线分支，只从 dev 合并，不打 commit
```

### 流程

1. `dev` 开发完成 → `git checkout main && git merge dev`
2. 在 `main` 上打 tag：`git tag -a vX.Y.Z -m "..."` + `git push`
3. 打包 → 创建 GitHub Release
4. 切回 `dev` 继续开发

**tag 必须打在 `main` 上，不要打在 `dev` 上。**

发布：`git tag -a vX.Y.Z -m "..."` + `git push origin vX.Y.Z`。

### Release 维护（释放 GitHub 存储空间）

GitHub Release 有 2GB 总大小限制，旧版本 `.exe` 会占满配额。**只删 exe 资产，保留 release 页面和 changelog：**

```bash
# ✅ 正确 — 只删 exe，release 页面和 tag 都保留
gh release delete-asset vX.Y.Z "OpenStart.Setup.X.Y.Z.exe" --yes

# ❌ 错误 — 删了整条 release + git tag，版本历史永久丢失
gh release delete vX.Y.Z --yes --cleanup-tag
```

每次新建 release 后，顺手清理上一个旧版本的 exe 即可。

## 已知问题

1. **图标更换**：替换 `src-tauri/icons/` 下的 PNG/ICO 后，需要 `cargo build` 重新编译（图标嵌入二进制）。文件名不要改，否则需同步 `tauri.conf.json` 的 `bundle.icon` 配置。

2. **Rust 编译冲突**：`cargo build` 前确保 `openstart.exe` 已退出（`taskkill /F /IM openstart.exe`），否则报 "拒绝访问" (os error 5)。

3. **CLI 输出**：release 构建是 `windows_subsystem = "windows"`，`main.rs` 在 `--cli` 模式下会 `AttachConsole(ATTACH_PARENT_PROCESS)` 并重开 `CONOUT$`；如果输出不可见，先用 `cmd` 启动再排查。

4. **单实例**：使用 `Local\OpenStart_SingleInstance`（每个登录会话一个 GUI）；CLI 进程不占用互斥量。

5. **MSVC D8050（编译 ring 失败）**：`cl.exe ... c1.dll ... 未能将命令行放入调试记录` 是本机 MSVC 环境问题，不是项目代码错误。优先用 VS 的 “x64 Native Tools Command Prompt for VS 2022” 运行；仍失败可在 PowerShell 设 `$env:CARGO_PROFILE_DEV_DEBUG="0"` 后重试，或直接 `npm run tauri:build`（release 默认无调试信息）。再不行就修复 VS C++ 工具链、给 VS 的 `HostX64\x64` 和 `src-tauri\target` 加杀软排除，并清理过长/损坏的 PATH。
