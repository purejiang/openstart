# OpenStart

Windows 开机自启动命令行管理器。支持 PowerShell、CMD、Git Bash、Windows Terminal，按分组批量执行。

## 功能

- 🔍 自动检测系统终端（PowerShell / CMD / Git Bash / Windows Terminal）
- 📋 命令增删改查，支持分组管理
- 🚀 分组批量执行：同组 WT 命令合并为一个窗口多标签页
- ⚡ 开机自启动 + 可配置延迟
- 🖥️ GUI + CLI 双模式

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
openstart --cli status
```

## License

MIT
