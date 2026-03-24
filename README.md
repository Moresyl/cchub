<div align="center">

<img src="app-icon.png" alt="CCHub" width="120" />

# CCHub

**Claude Code Full Ecosystem Management Platform**

[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Latest Release](https://img.shields.io/github/v/release/Moresl/cchub?color=green)](https://github.com/Moresl/cchub/releases)
[![Downloads](https://img.shields.io/github/downloads/Moresl/cchub/total?color=blue)](https://github.com/Moresl/cchub/releases)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Backend-red.svg)](https://rust-lang.org)
[![React](https://img.shields.io/badge/React-19-61DAFB.svg)](https://react.dev)

[中文](README.zh-CN.md) · English

**[Download Latest Release](https://github.com/Moresl/cchub/releases/latest)**

</div>

---

## Introduction

CCHub is a desktop application for managing the complete Claude Code ecosystem — MCP Servers, Skills, Plugins, Hooks, and Config Profiles — all in one place.

The Claude Code ecosystem is growing rapidly, but management is fragmented: manual JSON editing, manual file copying, no unified interface. CCHub is here to fix that.

## Screenshots

<!-- Add your screenshots here -->
<!-- ![MCP Server Management](screenshots/mcp-management.png) -->
<!-- ![Config Profiles](screenshots/config-profiles.png) -->
<!-- ![Dark Theme](screenshots/dark-theme.png) -->

> Screenshots coming soon. Star the repo to stay updated!

## Features

- **MCP Server Management** — Auto-scan installed MCP Servers (Claude Code, Claude Desktop, Cursor). Enable/disable, edit config, delete.
- **MCP Server Marketplace** — Built-in registry with categorized MCP servers, one-click install with env config, custom source support.
- **MCP Server Health Monitoring** — Command existence check, process spawn test, latency measurement.
- **Skills & Plugins** — Browse installed Skills and Plugins with trigger commands, descriptions, and file paths. MDXEditor rich-text editing.
- **Workflows Management** — 12 built-in workflow templates (Code Review, TDD, Bug Diagnosis, Refactoring, Security Audit, etc.). One-click install to Claude / Codex / Gemini / OpenCode / OpenClaw, Markdown editing, enable/disable toggle.
- **Hooks Management** — Visualize all Hooks with event types, matchers, and commands.
- **Config Profiles** — Switch between different configurations for Claude Code, Codex, Gemini CLI, OpenCode, OpenClaw. Structured editing with presets, auto-scan local configs.
- **CLAUDE.md Manager** — Visual editor for CLAUDE.md project instructions with templates.
- **Tools Page** — Claude Code permission slider (4 levels), StatusLine (claude-hud) install/config/toggle, Codex settings.
- **StatusLine (claude-hud)** — One-click install, proxy support, China mirror fallback, display configuration.
- **Security Audit** — Permission scanning for env secrets, shell execution risks, npx auto-install risks.
- **Backup & Restore** — Export all configs as SQL, import with legacy JSON support.
- **Auto Update** — Built-in update checker with Tauri native updater + GitHub Releases fallback.
- **Dark / Light Theme** — Glassmorphism UI with theme switching.
- **i18n** — Chinese (default) and English.
- **System Tray** — Minimize to tray on close.

## Download

| File | Description |
|------|-------------|
| [Setup EXE](https://github.com/Moresl/cchub/releases/latest) | **Recommended** — NSIS installer with shortcuts and auto-update |
| [MSI](https://github.com/Moresl/cchub/releases/latest) | Windows Installer format for enterprise deployment |
| [Portable](https://github.com/Moresl/cchub/releases/latest) | No install needed — double-click and run |
| [macOS DMG](https://github.com/Moresl/cchub/releases/latest) | Apple Silicon & Intel |
| [Linux](https://github.com/Moresl/cchub/releases/latest) | deb / AppImage / RPM |

**Requirements:** Windows 10/11 (x64), macOS 10.15+, Linux (WebKit2GTK required).

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop Framework | **Tauri 2.0** — Rust backend + Web frontend, 10x lighter than Electron |
| Frontend | **React 19 + TypeScript + Tailwind CSS 4** |
| Backend | **Rust** — High performance, single binary distribution |
| Database | **SQLite** (rusqlite) — Zero-dependency local storage |
| Build | **Vite 6 + pnpm** |

## Getting Started (Development)

### Prerequisites

- [Node.js](https://nodejs.org) >= 18
- [pnpm](https://pnpm.io) >= 8
- [Rust](https://rustup.rs) >= 1.70
- [Tauri 2.0 Prerequisites](https://v2.tauri.app/start/prerequisites/)

### Install & Run

```bash
# Clone
git clone https://github.com/Moresl/cchub.git
cd cchub

# Install dependencies
pnpm install

# Development
pnpm tauri dev

# Build
pnpm tauri build
```

## Scan Paths

CCHub auto-scans MCP Server configs from:

| Path | Description |
|---|---|
| `~/.claude/plugins/**/.mcp.json` | Claude Code plugin directory (recursive) |
| `%APPDATA%/Claude/claude_desktop_config.json` | Claude Desktop config |
| `~/.cursor/mcp.json` | Cursor editor config |

## Roadmap

- [x] MCP Server management (scan, toggle, edit, delete)
- [x] MCP Server marketplace (registry, one-click install, custom sources)
- [x] MCP Server health monitoring (command check, spawn test, latency)
- [x] Skills & Plugins browser (MDXEditor, cross-tool sync)
- [x] Workflows management (12 templates, Markdown editing, enable/disable)
- [x] Hooks visualization
- [x] Config Profiles (structured editor, multi-tool switching)
- [x] CLAUDE.md manager (editor, templates, toggle)
- [x] Tools page (permissions, StatusLine, Codex settings)
- [x] StatusLine (claude-hud) integration (install, config, proxy, China mirror)
- [x] Security audit (permission scanning, risk detection)
- [x] Backup & restore (SQL export/import)
- [x] Auto-update (Tauri Updater + GitHub fallback)
- [x] Dark / Light theme
- [x] i18n (Chinese + English)
- [x] macOS / Linux support (CI multi-platform builds)
- [x] System tray (minimize on close)
- [ ] Config change detection (security audit over time)
- [ ] Hooks editor (create/edit hooks from UI)

---

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Moresl/cchub&type=Date)](https://star-history.com/#Moresl/cchub&Date)

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Tauri](https://tauri.app) — Lightweight desktop app framework
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — AI coding assistant
- [MCP](https://modelcontextprotocol.io) — Model Context Protocol
