<div align="center">

<img src="app-icon.png" alt="CCHub" width="128" />

# CCHub

### Switch AI coding tool configurations from one desktop app.

[![GitHub Stars](https://img.shields.io/github/stars/Moresyl/cchub?style=social)](https://github.com/Moresyl/cchub/stargazers)
[![Latest Release](https://img.shields.io/github/v/release/Moresyl/cchub?color=green)](https://github.com/Moresyl/cchub/releases)
[![Downloads](https://img.shields.io/github/downloads/Moresyl/cchub/total?color=blue)](https://github.com/Moresyl/cchub/releases)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tauri 2.0](https://img.shields.io/badge/Tauri-2.0-orange.svg)](https://tauri.app)

**Windows** · **macOS** · **Linux** &nbsp;|&nbsp; [中文](README.zh-CN.md) · English

[**Download**](https://github.com/Moresyl/cchub/releases/latest) &nbsp;&nbsp;·&nbsp;&nbsp; [Report Bug](https://github.com/Moresyl/cchub/issues) &nbsp;&nbsp;·&nbsp;&nbsp; [Request Feature](https://github.com/Moresyl/cchub/issues)

</div>

---

## Purpose

CCHub opens directly into configuration switching. Filter and search saved provider profiles by tool, see which profile is active, and apply another in one click. You can also create, edit, duplicate, ping, and stream-check profiles. Shared providers and project profiles remain available as advanced options.

The compact neutral light/dark workspace uses a unified frameless desktop title bar, native window actions, a collapsible sidebar, and `Ctrl+K` quick switching. New profiles start from an official default or a blank custom template; existing saved profiles remain compatible. Configuration files, MCP servers, and Skills are secondary destinations. Former standalone pages such as Autopilot, session analytics, Marketplace, and security audit are no longer product entry points; upgrading does not delete existing configuration data.

---

## Workspace

| Dark theme                                          | Light theme                                           |
| --------------------------------------------------- | ----------------------------------------------------- |
| ![CCHub dark workspace](screenshots/dark-theme.png) | ![CCHub light workspace](screenshots/light-theme.png) |

Screenshots are from the desktop app; the example endpoint has been anonymized.

### Configuration editor

![CCHub configuration editor with structured options and syntax highlighting](screenshots/profile-editor.png)

---

## Features

| Feature              | Description                                                             |
| -------------------- | ----------------------------------------------------------------------- |
| **Config Profiles**  | Save and apply configurations for Claude Code, Codex, Gemini, and more  |
| **Providers**        | Tool filters, search, presets, ping, stream checks, and shared profiles |
| **Config Files**     | View and edit managed tool configuration files                          |
| **MCP Servers**      | Scan, edit, and sync MCP configurations across tools                    |
| **Skills & Plugins** | Browse, edit, and sync Skills across tools                              |
| **Quick Switch**     | `Ctrl+K` to find/apply profiles or navigate to configuration pages      |

### Platform

| Feature                | Description                                                                   |
| ---------------------- | ----------------------------------------------------------------------------- |
| **Cross-platform**     | Windows 10/11, macOS 10.15+, Linux                                            |
| **Dark / Light Theme** | Compact desktop UI with keyboard focus states and reduced-motion support      |
| **Backup & Restore**   | Export all configs as SQL, import with legacy format support                  |
| **Auto Update**        | Signed Tauri updater when available, with a reliable GitHub Releases fallback |
| **i18n**               | Chinese, English, Japanese                                                    |
| **System Tray**        | Minimize to tray on close                                                     |

---

## Download

| File                                                                       | Platform | Description                                                         |
| -------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------- |
| [`CCHub_x64-setup.exe`](https://github.com/Moresyl/cchub/releases/latest)  | Windows  | **Recommended** — branded bilingual NSIS installer with auto-update |
| [`CCHub_x64_en-US.msi`](https://github.com/Moresyl/cchub/releases/latest)  | Windows  | Branded MSI format for enterprise deployment                        |
| [`CCHub_aarch64.dmg`](https://github.com/Moresyl/cchub/releases/latest)    | macOS    | Apple Silicon (M1/M2/M3/M4)                                         |
| [`CCHub_x64.dmg`](https://github.com/Moresyl/cchub/releases/latest)        | macOS    | Intel                                                               |
| [`CCHub_amd64.deb`](https://github.com/Moresyl/cchub/releases/latest)      | Linux    | Debian / Ubuntu                                                     |
| [`CCHub_amd64.AppImage`](https://github.com/Moresyl/cchub/releases/latest) | Linux    | Universal AppImage                                                  |
| [`CCHub_x86_64.rpm`](https://github.com/Moresyl/cchub/releases/latest)     | Linux    | Fedora / RHEL                                                       |

---

## Tech Stack

| Layer      | Technology                                                                     |
| ---------- | ------------------------------------------------------------------------------ |
| Framework  | [**Tauri 2.0**](https://tauri.app) — Rust backend + Web frontend, ~20MB binary |
| Frontend   | **React 19** + **TypeScript** + **Tailwind CSS 4**                             |
| Backend    | **Rust** — High perf, memory safe, single binary                               |
| Database   | **SQLite** (rusqlite) — Zero-dependency local storage                          |
| Build      | **Vite 6** + **pnpm**                                                          |
| Data Layer | **TanStack React Query** — Unified caching & state                             |
| UI         | CCHub design system + **Tailwind CSS 4** + **cmdk** + **Lucide**               |

---

## Development

### Prerequisites

- [Node.js](https://nodejs.org) >= 20
- [pnpm](https://pnpm.io) 10.32.1
- [Rust](https://rustup.rs) stable
- [Tauri 2.0 Prerequisites](https://v2.tauri.app/start/prerequisites/)

### Quick Start

```bash
git clone https://github.com/Moresyl/cchub.git
cd cchub
pnpm install
pnpm tauri dev
```

### Build

```bash
pnpm tauri build
```

Windows installer artwork, localization, and validation are documented in [docs/WINDOWS_INSTALLER.md](docs/WINDOWS_INSTALLER.md).

---

## Supported Config Sources

CCHub auto-scans MCP server configs from:

| Path                                           | Source                                      |
| ---------------------------------------------- | ------------------------------------------- |
| `~/.claude/plugins/**/.mcp.json`               | Claude Code plugins (recursive)             |
| `%APPDATA%/Claude/claude_desktop_config.json`  | Claude Desktop                              |
| `~/.cursor/mcp.json`                           | Cursor                                      |
| `~/.codex/config.toml`                         | Codex CLI                                   |
| `~/.gemini/settings.json`                      | Gemini CLI                                  |
| `~/.hermes/cli-config.yaml` + `~/.hermes/.env` | Hermes Agent (NousResearch) — YAML + dotenv |

---

## Contributing

Contributions welcome! See [issues](https://github.com/Moresyl/cchub/issues) for ideas.

```
Fork → Branch → Commit → Push → Pull Request
```

---

## Star History

<div align="center">

If CCHub saves you time, consider giving it a star. It helps others discover the project.

[![Star History Chart](https://api.star-history.com/svg?repos=Moresyl/cchub&type=Date)](https://star-history.com/#Moresyl/cchub&Date)

</div>

---

## License

MIT License — see [LICENSE](LICENSE) for details.

## Acknowledgments

- [Tauri](https://tauri.app) — Lightweight desktop framework
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — AI coding assistant
- [MCP](https://modelcontextprotocol.io) — Model Context Protocol
