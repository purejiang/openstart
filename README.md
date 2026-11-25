# OpenStart

Windows 开机自启动命令行管理器。支持 PowerShell、CMD、Git Bash、Windows Terminal，按分组批量执行。

## 功能

- 🔍 自动检测系统终端（PowerShell / CMD / Git Bash / Windows Terminal）
- 📋 命令增删改查 + 实时搜索，支持分组管理
- 🚀 分组批量执行：同组 WT 命令合并为一个窗口多标签页
- ⚡ 开机自启动 + 可配置延迟
- 🖥️ GUI + CLI 双模式，CLI 支持 `run / status / stop`
- 🛡️ 单实例运行；同一命令/分组运行期间拒绝重复启动
- ✅ 命令启用/停用：行内开关一键切换；禁用命令不能被单条、分组、批量或开机自启调起
- 🧭 进程跟踪：显示 PID、运行状态、运行时长，可一键停止（含进程树）
- 🔗 多步链式命令：支持分步骤配置，自动拼接延迟分隔符
- ↕️ 步骤拖拽排序：拖拽重排命令步骤，直观调整执行顺序
- 📝 命令备注：命令和步骤均支持备注，列表中一目了然
- ⏱️ 执行时间追踪：自动记录每次执行时间，随时查看

## 技术栈

React 19 + TypeScript + Vite / Rust + Tauri v2 / SQLite

## 安装

从 [Releases](https://github.com/purejiang/openstart/releases) 下载 setup.exe 安装。

## 开发

```bash
npm install
npm run tauri:dev    # 开发
npm run tauri:build  # 打包
```

## CLI

```powershell
openstart --cli list
openstart --cli add "dev" "npm run dev" powershell --auto
openstart --cli exec terminal:Git\ Bash "cd /d/project && npm start"
openstart --cli run <command-id> [--force]
openstart --cli group "Dev" [--force]
openstart --cli run-all [--force]
openstart --cli status                 # 活动运行 + App 自启状态
openstart --cli status <run-id>        # 单次运行的详情
openstart --cli status --all           # 最近 100 条记录
openstart --cli stop <run-id>
openstart --cli stop --command <command-id>
openstart --cli stop --group "Dev"
openstart --cli stop --all
```

- `run/group/exec` 启动成功会打印 run id 与 PID；Windows Terminal 的 PID 会在约 2 秒内解析，超时显示 `pid pending`。
- 重复启动运行中的命令/分组会被拒绝，退出码 `2`；`--force` 会先停止旧实例再启动。
- 禁用命令（`[disabled]`）不会被 `run` 启动；`group`/`run-all` 会跳过禁用成员。
- 退出码：`0` 成功，`1` 错误，`2` already running。
- 停止使用 `taskkill /T /F`，会结束整个进程树。

## 运行状态与去重

- 同一个命令、同一分组或同一 ad-hoc 命令在运行期间只能存在一个实例（GUI、CLI、开机自启共用 SQLite 中的 `runs` 注册表）。
- GUI 每 2.5 秒刷新一次状态；关闭终端窗口或执行 `stop` 后状态自动变为 `exited` / `stopped`。
- 开启“Keep terminal open after execution”时，命令执行完但终端未关仍视为运行中，直到关窗或 Stop。
- 运行记录保存在 `%APPDATA%\openstart\openstart.db` 的 `runs` 表，最多保留最近 500 条已完成记录。
- 可用 `OPENSTART_DATA_DIR` 环境变量覆盖数据目录（便于测试或便携运行）。
- Windows Terminal 的 PID 通过命令行的唯一 marker 解析；极少数无法解析的情况会显示 `unknown`。

## License

MIT
