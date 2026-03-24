<div align="center">

<img src="app-icon.png" alt="CCHub" width="120" />

# CCHub

**Claude Code 全生态管理平台**

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Latest Release](https://img.shields.io/github/v/release/Moresl/cchub?color=green)](https://github.com/Moresl/cchub/releases)
[![Downloads](https://img.shields.io/github/downloads/Moresl/cchub/total?color=blue)](https://github.com/Moresl/cchub/releases)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Backend-red.svg)](https://rust-lang.org)
[![React](https://img.shields.io/badge/React-19-61DAFB.svg)](https://react.dev)

中文 · [English](README.md)

**[下载最新版本](https://github.com/Moresl/cchub/releases/latest)**

</div>

---

## 简介

CCHub 是一个桌面应用程序，用于统一管理 Claude Code 的完整生态系统——MCP 服务、技能、插件、钩子、配置切换，一站式搞定。

当前 Claude Code 生态爆炸式增长，但管理体验极度碎片化：手动编辑配置文件、手动复制文件、没有统一界面。CCHub 就是为了解决这个问题而生。

## 截图

<!-- 在此添加截图 -->
<!-- ![MCP 服务管理](screenshots/mcp-management.png) -->
<!-- ![配置切换](screenshots/config-profiles.png) -->
<!-- ![暗色主题](screenshots/dark-theme.png) -->

> 截图即将上线，Star 关注获取最新动态！

## 核心功能

- **MCP 服务管理** — 自动扫描所有已安装的 MCP 服务（Claude Code、Claude 桌面版、Cursor），支持启用/禁用、编辑配置、一键删除
- **MCP 服务市场** — 内置分类注册表，一键安装并配置环境变量，支持自定义源
- **MCP 服务健康监控** — 命令存在性检测、进程启动测试、延迟测量
- **技能与插件管理** — 浏览已安装的技能和插件，MDXEditor 富文本编辑，跨工具同步
- **工作流管理** — 内置 12 个热门工作流模板（代码审查、TDD、Bug 诊断、重构、安全审计等），一键安装到 Claude / Codex / Gemini / OpenCode / OpenClaw，支持 Markdown 编辑、启用/禁用
- **钩子管理** — 可视化查看所有钩子配置，包括事件类型、匹配器、执行命令
- **配置切换** — 在 Claude Code、Codex、Gemini CLI、OpenCode、OpenClaw 等不同配置间一键切换，支持结构化编辑与预设模板，自动扫描本地已有配置
- **CLAUDE.md 管理器** — 可视化编辑项目指令文件，支持模板创建
- **工具页面** — Claude Code 权限滑块（4 档）、StatusLine (claude-hud) 安装/配置/开关、Codex 设置
- **StatusLine (claude-hud)** — 一键安装、代理下载支持、国内镜像回退、显示选项配置
- **安全审计** — 环境变量敏感信息扫描、Shell 执行风险检测、npx 自动安装风险检测
- **备份与恢复** — 导出所有配置为 SQL 文件，支持导入（兼容旧版 JSON 格式）
- **自动更新** — 内置版本检查，Tauri 原生更新 + GitHub Releases 回退
- **深色/浅色主题** — 毛玻璃质感的精致界面，支持主题切换
- **中英双语** — 默认中文界面，一键切换英文
- **系统托盘** — 关闭窗口最小化到托盘

## 下载安装

| 文件 | 说明 |
|------|------|
| [安装版 EXE](https://github.com/Moresl/cchub/releases/latest) | **推荐** — NSIS 安装包，支持快捷方式和自动更新 |
| [MSI 安装版](https://github.com/Moresl/cchub/releases/latest) | Windows Installer 格式，适合企业部署 |
| [便携版](https://github.com/Moresl/cchub/releases/latest) | 免安装，双击即用，可放 U 盘随身携带 |
| [macOS DMG](https://github.com/Moresl/cchub/releases/latest) | Apple Silicon 和 Intel 版本 |
| [Linux](https://github.com/Moresl/cchub/releases/latest) | deb / AppImage / RPM |

**系统要求：** Windows 10/11 (x64)、macOS 10.15+、Linux（需要 WebKit2GTK）。

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | **Tauri 2.0** — Rust 后端 + Web 前端，比 Electron 轻 10 倍 |
| 前端 | **React 19 + TypeScript + Tailwind CSS 4** |
| 后端 | **Rust** — 高性能，单文件分发 |
| 数据库 | **SQLite**（rusqlite）— 零依赖本地存储 |
| 构建工具 | **Vite 6 + pnpm** |

## 开发指南

### 环境要求

- [Node.js](https://nodejs.org) >= 18
- [pnpm](https://pnpm.io) >= 8
- [Rust](https://rustup.rs) >= 1.70
- [Tauri 2.0 前置依赖](https://v2.tauri.app/start/prerequisites/)

### 安装运行

```bash
# 克隆仓库
git clone https://github.com/Moresl/cchub.git
cd cchub

# 安装依赖
pnpm install

# 开发模式
pnpm tauri dev

# 构建发布
pnpm tauri build
```

## 扫描路径

CCHub 会自动扫描以下位置的 MCP 服务配置：

| 路径 | 说明 |
|---|---|
| `~/.claude/plugins/**/.mcp.json` | Claude Code 插件目录（递归） |
| `%APPDATA%/Claude/claude_desktop_config.json` | Claude 桌面版配置 |
| `~/.cursor/mcp.json` | Cursor 编辑器配置 |

## 路线图

- [x] MCP 服务管理（扫描、启停、编辑、删除）
- [x] MCP 服务市场（分类注册表、一键安装、自定义源）
- [x] MCP 服务健康监控（命令检测、启动测试、延迟测量）
- [x] 技能与插件浏览（MDXEditor 富文本编辑、跨工具同步）
- [x] 工作流管理（12 个模板、Markdown 编辑、启用/禁用）
- [x] 钩子可视化
- [x] 配置切换（结构化编辑器、多工具切换）
- [x] CLAUDE.md 管理器（编辑器、模板、开关）
- [x] 工具页面（权限控制、StatusLine、Codex 设置）
- [x] StatusLine (claude-hud) 集成（安装、配置、代理、国内镜像）
- [x] 安全审计（权限扫描、风险检测）
- [x] 备份与恢复（SQL 导出/导入）
- [x] 自动更新（Tauri 更新器 + GitHub 回退）
- [x] 深色/浅色主题
- [x] 中英文国际化
- [x] macOS / Linux 支持（CI 多平台构建）
- [x] 系统托盘（关闭最小化到托盘）
- [ ] 配置变更检测（安全审计时间线）
- [ ] 钩子编辑器（从界面创建/编辑钩子）

---

## 参与贡献

欢迎提交 Pull Request！

1. Fork 本仓库
2. 创建功能分支（`git checkout -b feature/amazing-feature`）
3. 提交更改（`git commit -m '添加新功能'`）
4. 推送分支（`git push origin feature/amazing-feature`）
5. 发起 Pull Request

## Star 趋势

[![Star History Chart](https://api.star-history.com/svg?repos=Moresl/cchub&type=Date)](https://star-history.com/#Moresl/cchub&Date)

## 许可证

本项目采用 MIT 许可证 — 详见 [LICENSE](LICENSE) 文件。

## 致谢

- [Tauri](https://tauri.app) — 轻量级桌面应用框架
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — AI 编程助手
- [MCP](https://modelcontextprotocol.io) — 模型上下文协议
