# AGENTS.md

## 项目概览

Windows 开机自启动命令行管理器。

## 关键命令

```bash
npm run tauri:dev     # 启动开发（Vite + Rust 编译 + GUI 窗口，长时间运行）
npm run tauri:build   # 生产打包 → NSIS 安装器
npm run build         # 仅前端构建（tsc + vite）
npm run lint          # oxlint
cd src-tauri && cargo test    # Rust 单元测试（8 个）
cd src-tauri && cargo check   # Rust 类型检查
```

**注意**：`cargo test` 仅测试 lib crate 的单元测试；main.rs 没有测试。


## 技术栈

**Tauri v2 + React 19 + SQLite**

## 架构

### 后端模块 (src-tauri/src/)

| 文件 | 职责 | 注意 |
|---|---|---|
| `storage.rs` | SQLite CRUD、`Command`/`CommandStep` 结构、DB 迁移 | `Mutex<Connection>` 满足 Tauri `State: Send+Sync` |
| `commands.rs` | 所有 Tauri 命令、终端启动、多步链式脚本拼接 | **最大文件 ~475 行**，加代码时慎重 |
| `terminal.rs` | 终端检测、WT settings.json 解析、bash 路径查找 | 纯检测逻辑，无副作用 |
| `main.rs` | CLI 入口（`openstart --cli`） | 使用 `commands::build_spawn_args` |
| `lib.rs` | Tauri 应用启动、托盘、自启执行、invoke_handler 注册 | 关闭窗口 → 隐藏到托盘，不退出 |
| `autostart.rs` | HKCU Run 注册表操作 | Windows only |
| `settings.rs` | JSON 配置文件读写（`app_autostart`、`startup_delay_seconds`） | 数据目录：`%APPDATA%\openstart\` |

### 前端 (src/)

| 文件 | 职责 | 注意 |
|---|---|---|
| `App.tsx` | 分组命令表、步骤编辑器 Modal、设置页 | **保持精简** |
| `App.css` | Code Dark (#0F172A) + Run Green (#22C55E) 设计系统 | CSS 变量：`--bg-primary` 等 |
| `api.ts` | `invoke()` 包装，类型化调用 Tauri 命令 | 新增命令需两端同步 |
| `types.ts` | TypeScript 接口（`Command`、`CommandStep`、`AppSettings` 等） | 必须与 Rust 结构体字段一致 |

### 四条执行路径

1. **GUI 点击 ▶** → `executeCommandById(id)` → 从 DB 加载 → 解析 steps → 链式脚本 → spawn
2. **GUI 分组 Run** → `executeGroup(name)` → 每条 cmd 解析 steps → 同组 WT 合并窗口/混合终端分开
3. **CLI run/run-all** → `main.rs` → 用共享的 `build_spawn_args` + steps 解析
4. **开机自启** → `lib.rs` setup → 延迟后 `spawn_terminal_batch`（全部用 WT）

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
