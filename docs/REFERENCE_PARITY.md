# CCHub feature validation

This document tracks user-visible behavior and native verification. A row is complete only after its workflow and relevant tests pass.

| Area | CCHub evidence | Status / acceptance gap |
| --- | --- | --- |
| Workspace shell | `src/components/layout/{Sidebar,Header}.tsx`, `src/styles/{_layout,_components}.css`, `src/lib/theme.ts` | Complete: resizable sidebar, usable collapsed icon navigation, command switcher, responsive layout and System/Light/Dark themes are present. |
| Provider profiles and presets | `src/pages/Profiles.tsx`, `src/lib/configProfiles/presets/`, `src-tauri/src/commands/extra/config_profiles/` | Partial: built-in presets, search, reorder, apply, shared providers, endpoint and stream checks are present. Verify profile import/export, native tray clicks and real tool configuration round trips. |
| Claude Desktop provider management | `src/pages/ClaudeDesktop.tsx`, `src-tauri/src/commands/claude_desktop_profiles.rs`, `src-tauri/src/provider_proxy/desktop.rs` | Complete: direct gateways plus authenticated local proxy mode, format conversion, explicit model routing, response model restoration, OS credential storage, provider deep links, rollback and backup, MCP configuration and synchronization are present. |
| MiniMax Code | `src/pages/Mcode.tsx`, `src-tauri/src/commands/mcode_commands.rs`, `src-tauri/src/mcp/mcode.rs` | Complete: independent custom-provider CRUD, default-model protection, MCP synchronization, Skills path detection and provider deep-link import are present. |
| Unified MCP | `src/pages/McpServers.tsx`, `src-tauri/src/mcp/{config,formats}.rs` | Partial: managed tools have native mappings. STDIO/HTTP/SSE creation, headers, disabled-entry filtering and target-specific formats have automated coverage; verify full bidirectional writes against real installations. |
| Prompts and Skills | `src/pages/{Prompts,Skills}.tsx`, `src-tauri/src/commands/{prompt_library,skill_repository_commands}.rs` | Partial: per-app prompt library, live files, repository/ZIP Skills, copy/symlink and repositories exist. Verify external-edit backfill, tool-specific paths and restore behavior. |
| Local proxy and failover | `src-tauri/src/provider_proxy/`, `src/pages/ProxyAdvanced.tsx` | Complete: per-app takeover, format rewrites, circuit breakers, failover and optimizer controls exist. Native hot enable/disable, persisted restart restore, authentication rejection and fixture requests passed. |
| Usage and sessions | `src/pages/{Usage,Sessions}.tsx`, `src-tauri/src/commands/usage_analytics.rs` | Partial: proxy aggregates, trends, detailed logs and pricing exist. Sessions route all managed tools; fixture reads, resume commands and cumulative-token totals passed native checks. |
| Backup, cloud and deep links | `src/components/{SettingsImportExportSection,WebDavSyncSection,S3SyncSection,DeepLinkImportDialog}.tsx` | Partial: SQL export/restore, managed backups, WebDAV/S3 and provider/MCP/prompt/skill deep links exist. Verify cloud rollback and all app targets against fixtures. |
| Platform and release | `src-tauri/tauri.conf.json`, `.github/workflows/release.yml` | Partial: tray, autostart, updater and zh/en/ja exist. Additional locale and native cross-platform package verification remain. |

Configuration switching remains the primary workspace. Navigation cleanup must never delete stored user data or tool configuration files.
