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

The compact neutral light/dark workspace uses a unified frameless desktop title bar, native window actions, a collapsible sidebar, and `Ctrl+K` quick switching. New profiles start from an official default or a blank custom template; existing saved profiles remain compatible. Configuration files, MCP servers, and Skills are secondary destinations. The Runtime sidebar view provides proxy management, usage analytics, and session browsing. Former standalone pages such as Autopilot, Marketplace, and security audit are no longer product entry points. Upgrading does not delete existing configuration data.

---

## Workspace

| Dark theme                                          | Light theme                                           |
| --------------------------------------------------- | ----------------------------------------------------- |
| ![CCHub dark workspace](screenshots/dark-theme.png) | ![CCHub light workspace](screenshots/light-theme.png) |

Screenshots are from the desktop app; the example endpoint has been anonymized.

### Configuration editor

![CCHub configuration editor with structured options and syntax highlighting](screenshots/profile-editor.png)

Model discovery follows the selected API protocol and retains provider-reported context/output limits, modalities, native endpoints, and reasoning levels. Catalogs are saved with their profile and invalidated when its connection changes. OpenCode model edits remain separate when switching models; reported limits and modalities can be applied explicitly, and clearing token limits restores the tool defaults. Discovery uses a single 15-second deadline across supported pagination, with limits of 50 pages, 10,000 model rows, and 8 MiB.

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
| **Native Configs**   | OpenCode switching preserves JSONC comments, MCP, plugins, and other providers |
| **Sessions & Usage** | Browse native sessions and import output, reasoning, and cache usage without duplicate billing |

### Local proxy and failover

Advanced proxy settings support separate routing policies for Claude, Codex, Gemini, Grok Build, OpenCode, OpenClaw and Hermes. Groups can prioritize members in order, rotate the starting member per request, or select one fixed profile. Nested groups retain their own strategy and repeated profiles are deduplicated; cycles and more than eight group levels are rejected. The current active profile is retained. Rules run top to bottom and combine model matching, images, reasoning and minimum request bytes. Unmatched requests use the default group or the existing active-profile order.

Preview draft rules locally, including a simulated byte size, without sending model requests, saving settings or advancing rotation. Saving checks the exact loaded revision; conflicts retain the draft and never retry an overwrite. Routing stays within the selected group and still respects retry budgets and circuit breakers. A deleted selected member stops routing with an error instead of silently choosing a profile outside the group.

Conversation routing defaults to off, with automatic, session and tool-turn modes available. Only explicit client session identifiers are used; identical prompts do not merge conversations. Tools, credentials, models, reasoning settings and routing policies are isolated. Automatic mode retains tool continuations or a successful response reporting at least 1024 cached input tokens for five minutes; session mode retains bindings for up to 24 hours. Only successful replies update bindings, and streams must deliver their completion event. Managed accounts stay pinned through default-account changes, while invalid logins, configuration changes and circuit breakers still apply. Bindings live in bounded memory without changing profile files; this does not guarantee encrypted reasoning history can be replayed across providers.

Alternate endpoints are attempted in order. A profile records one failure after its available endpoints are exhausted. Disabling cross-profile failover or exhausting its retry budget preserves the vendor's HTTP status, error body, and `Retry-After`. Open circuits are not bypassed; when every candidate is blocked, the proxy returns HTTP 503 with a retry delay.

Recovery allows one probe at a time, and cancellation releases its slot. Health is recorded after the response body finishes; streams also check vendor error events and completion markers, preserving split Unicode characters and handling CRLF boundaries. Closing a response after reading its complete terminal event still records success and releases the recovery slot. Cancelling before the translated terminal event is read remains cancellation, even if the upstream event has already completed. Chat, Responses, and Gemini adapters forward errors rather than fabricate normal completion after interruption. Late requests cannot change a newer circuit state after a reset or another opening.

Messages, Responses, Chat and Gemini passthrough streams emit protocol-specific error events when cut short or interrupted. Passthrough errors name the profile without exposing raw request URLs or credentials. Terminal-event headers are checked separately, so payloads larger than 1 MiB cannot disable completion checks; a transport closure after a whole terminal event does not turn a completed reply into a failure. Partial Gemini input, output, reasoning and cache readings merge as cumulative counters, retain reported usage after interruption, and do not count repeated chunks twice.

Streaming usage is captured before protocol conversion, so metadata events omitted by an adapter still reach request accounting. Cancellation and interruption retain native counters and the reported model already read. Gemini candidate and reasoning counters combine even when delivered in separate events; missing or smaller later readings cannot erase earlier values. A total without its input split does not imply output usage.

Usage statistics follow the final request outcome. Failover does not count the same request twice. Interrupted or errored streams are recorded as failures; cancellation is recorded as 499 while retaining usage already reported. Request details and daily totals are saved together so a failed statistics write cannot leave a partial update.

Request deadlines are configured in the advanced proxy settings. Ordinary responses have a per-attempt total deadline covering request transmission, headers, and body (600 seconds by default). Streaming requests allow 60 seconds from transmission to the first raw byte and 120 seconds of upstream inactivity; heartbeats keep the stream alive. A stalled endpoint can fail over before client headers are committed. Set a timeout to `0` to disable it; values above 86400 seconds are rejected. These deadlines do not include credential acquisition or reading the incoming client body.

### Native configuration and usage sync

OpenCode uses the existing `opencode.jsonc` or `opencode.json`. Profiles retain the native provider ID and selected model, SDK extension options, and other model definitions. Applying a profile updates its provider and default model. Invalid syntax, duplicate fields, or an external change detected before writing stops the update with an actionable error.

Choose Runtime → Sessions → Sync usage to import completed requests from OpenCode V1/V2 databases. Totals include cache reads, cache writes, reasoning output, and compaction requests; unfinished responses remain eligible for the next sync. Corrected accounting updates existing records. Migration or moving the database does not import the same requests again. Durable identities survive log cleanup and are included in SQL backups. A result dialog lists errors when some sources could not sync.

### Provider balance and quota

The profile usage dialog shows every reported currency and quota window, including zero balances. Unknown units and missing usage metrics remain unknown; a balance alone does not imply a quota percentage. Built-in balance and Coding Plan queries route only exact official HTTPS hosts; an explicit Coding Plan selection can choose its fixed vendor endpoint. Generic relay queries share a 20-second budget across fallback endpoints, while official queries allow 15 seconds. JSON responses are limited to 2 MiB and credentials never follow redirects to another origin.

Refreshing after a network failure retains the same configuration's last successful reading with a visible stale-data notice. Credential failures replace the result. Switching profiles, tools or configuration invalidates previous readings and late requests. The expandable read-only JSON view supports syntax highlighting and scrolling through long responses.

### Protocol reply compatibility

Claude protocol conversion gives whole and streamed Chat Completions, Responses and Gemini replies Anthropic message IDs, preserving their tool-call IDs. Missing upstream message IDs receive independent random IDs. Responses streams emit one message start even when the upstream start is absent or repeated, and stop once on completion. Native Anthropic message IDs remain unchanged.

Gemini tool results match historical calls by ID and use the original function name, including parallel calls to the same function and reordered results. Missing reply call IDs receive UUIDs; tool failures retain an error result. Unmatched or repeated history IDs are rejected locally with an API error. Duplicate reply IDs stop conversion without emitting a second tool call or a normal completion, and conversion failures retain reported usage while counting as failures. Tool identity tracking is limited to 4096 calls and 1024 bytes per ID; these limits do not cover the entire parser or reasoning-signature history.

Whole and streamed Gemini replies share usage parsing rules, preserving valid input, output, reasoning and cache readings even without a total token count. Whole replies include reported reasoning tokens in output usage. Accounting keeps the upstream model name, and failover after a conversion failure does not count the request twice.

Responses conversion also supports standard reasoning summary/text events and data-only event types. Reasoning parts keep separate block identities, close before text or tools, and avoid replaying completed snapshots. Final-only parts are recovered when deltas are absent. Reasoning tracking has part and identity limits; adapter errors count as failed requests even when the vendor completed its reply. These limits apply to reasoning state, not the entire stream parser.

Ordinary tools converted to Responses explicitly keep non-strict behavior. Client-provided `strict: true/false` is preserved in both Responses and Chat Completions conversion, without turning optional parameters into required ones. Hosted web search does not receive the function-only `strict` field. Native protocol tool definitions remain unchanged.

### OAuth account status

Claude proxy requests using Codex OAuth, xAI OAuth or GitHub Copilot carry the resolved account and sign-in revision. Unbound profiles follow the default account; explicitly bound profiles retain their chosen account. Codex request account headers use that resolved account too. After account removal or a new sign-in, old requests cannot promote endpoint preferences or emit profile-failover notifications. Grok Build's xAI OAuth proxy uses the same account checks.

Concurrent GitHub Copilot token refreshes share one request. Old results cannot overwrite a new sign-in or restore a removed account. Account persistence uses atomic file replacement; failed saves retain the previous accounts, default selection and cached credentials. Token responses are limited to 2 MiB; empty, expired and invalid responses are not cached, and token endpoint error bodies are not displayed.

Explicitly rejected refresh credentials leave Codex OAuth and xAI OAuth accounts marked as requiring sign-in on this device. Ordinary network or proxy-challenge failures do not clear the account. Old refresh responses cannot overwrite a new sign-in or restore a removed account; cancelling device-code sign-in also cancels the backend flow so later authorization cannot commit. The Codex account panel keeps expired accounts visible with a sign-in action, offers retries for quota failures, and isolates late readings by account.

Codex OAuth quota/model queries and xAI OAuth model queries recover from an HTTP 401 by refreshing and retrying once for the original account. A second rejection marks only that current login as requiring sign-in. Changing the default account does not redirect an in-flight query, and late rejections cannot evict a newer cached token. HTTP 403, rate limits and server/network failures remain query failures. The recovery flow has a 45-second total deadline and a streaming 2 MiB response limit; redirects stay within the initial origin and vendor error bodies are not displayed. xAI quota remains unavailable through this integration and is not estimated.

### Balance and quota alerts

Enable alerts in a profile’s usage and balance dialog. Monitoring defaults off and checks approximately every five minutes while CCHub runs; it uses that profile’s configured usage script or provider API. Quota thresholds and balance thresholds are separate, with balances matched by currency or credit unit. Changing the query account pauses monitoring until the settings are saved again. Failed or stale readings do not trigger alerts.

The titlebar notification center retains up to 200 local history entries, supports marking them read, and offers manual checks. Known quota windows alert once per window; windows without reset times and recharged balances use hysteresis to prevent repeated alerts near the threshold. Optional OS notifications retry failed submissions up to six times. “Submitted to system” means the OS accepted the request, not that the user saw it. An interruption between system submission and storing the acknowledgement can result in a repeated OS notification; the in-app event is stored before submission. Cloud restoration preserves this device’s alert settings and history. Local loopback usage endpoints connect directly, so system proxy bypass settings do not interfere with local relays; remote endpoints retain the existing proxy behavior.

### Cloud backup settings

WebDAV passwords and S3 secrets stay in the OS keyring and are bound to their server and account. Changing either requires the corresponding credentials; switching back can reuse that account's saved credential. Save edited settings before reading, uploading, or restoring remote backups. Returning to a visible window refreshes saved settings while the form is idle and unchanged. Background refreshes preserve drafts, discard obsolete responses after editing or saving, and stop updating a closed page. Changing the S3 signing region clears cached remote status; a custom endpoint can retain its saved backup password when the account and backup location are unchanged. Restoring asks before replacing the local database, locks competing actions while confirmation is open, and permits retry after cancellation.

Cloud restoration keeps this device's cloud accounts and backup-password settings, custom tool directories, proxy, window and terminal preferences, and existing workspace directory bindings. Shared libraries such as provider fragments, common configuration, universal providers, skill repositories and request optimization settings are restored from the backup. New workspaces need local directory selection. Project files remain pending migration even when their original path exists on this device. Completion updates sync status only for the unchanged backup location. Uploads and restores are serialized across both cloud adapters. Local SQL-file imports still restore the full archive.

Cloud restores, local SQL imports and managed-backup restores clear old configuration query caches and refresh the migration summary, pending projects and local paths. The import summary shows restored counts and the safety-backup location; pending project mappings open automatically. A view-refresh failure is reported separately from an already completed restore.

Set and save a backup password of at least 12 characters before uploading. It is stored separately from the server login and scoped to the server, account, and backup location. New backups use AES-256-GCM with PBKDF2-SHA256 and a random salt. Other devices need the same backup password to restore. Keep it safe, and retain old passwords if you change it: earlier encrypted backups still require their original password. Disable automatic uploads before removing the saved backup password.

Downloads stop at 64 KiB for manifests and 15 MiB of backup content (encrypted files allow 68 additional bytes for format overhead). Restoring checks paths, declared sizes, SHA-256, and authenticated decryption before import. An incorrect password or modified encrypted content stops the import. Legacy unencrypted backups require explicit consent; older WebDAV backups without a digest still receive size checks. Each upload uses a separate snapshot filename.

Uploads use conditional writes: snapshots are created only if absent, and manifests replace only the strong ETag revision just read. A change during review or upload stops the operation without an unconditional retry. When first connecting to an existing backup or discovering another device's revision, restore it first or explicitly confirm replacement during manual upload; confirmation applies only to that exact revision. Automatic uploads cannot replace an unaccepted revision, and deleted backups require manual confirmation before recreation. Successful uploads and restores record this device's accepted revision in the OS keyring, outside database backups. Storage without strong ETags or conditional-write support remains available for downloads but stops uploads. Replacing the manifest does not delete previous snapshot files.

Backup imports validate tool identifiers, relative paths and encoded content, rejecting traversal, Windows device names, malformed content and descendant symbolic links or junctions below the selected root. Original files are staged before writing. A tool, artifact or database-update failure rolls back earlier file changes and retains the original database. If rollback itself fails, original files and a recovery path map are retained and their location is reported. Database installation retains the existing connection, so other operations cannot read a temporary empty database. Project-path migration also rolls back path records and file changes on failure; automatic recovery after process interruption is not implemented.

SQL backups load into a temporary database using the application's existing tables and indexes. External database access, schema replacement, triggers and SQL functions are refused. Invalid data or a timeout rolls back the SQL import, and errors do not echo backup contents.

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
