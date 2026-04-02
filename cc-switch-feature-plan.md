# CCHub 功能补齐计划（5 项）

## Context

对比 cc-switch v3.12.3，CCHub 核心功能已基本补齐，但仍缺少以下 5 项功能。本次计划逐项实现。

---

## 1. Common Config Snippet（公共配置片段）

**目标**：跨 Provider 共享的公共配置（如 Hide Attribution、Teammates、Effort Level、Tool Search 等），应用 Provider 时自动叠加。

**cc-switch 做法**：运行时 overlay，Common Config 不写入 Provider 本身，而是在切换 Provider 写入配置文件时动态合并。

### 修改文件

**后端**：
- `src-tauri/src/commands/extra_commands.rs`
  - 新增 `get_common_config_snippet` / `set_common_config_snippet` 命令
  - 数据存储在 `app_settings` 表的 JSON 字段中（如 `common_config_snippets`），按 tool_id 分组
  - `apply_config_profile` 命令中，写入配置文件前合并 Common Config 字段

- `src-tauri/src/lib.rs` — 注册新命令

**前端**：
- `src/pages/Profiles.tsx`
  - 新增 Common Config 编辑面板（按当前 App 显示可配置项）
  - 字段列表：`hideAttribution`、`enableTeammates`、`effortLevelHigh`、`enableToolSearch`、自定义 key-value
  - 切换 Provider 时前端提示已叠加公共配置

### 数据结构

```json
{
  "claude": { "hideAttribution": true, "enableTeammates": true },
  "codex": { "effortLevelHigh": true },
  "gemini": {}
}
```

---

## 2. Codex TOML 结构化编辑

**目标**：ConfigFiles 页为 `.codex/config.toml` 提供表单式编辑器，覆盖常用字段，同时保留原始 TOML 编辑器。

**参考**：Tools.tsx 中 Codex 1M Context Window 的读写模式（`read_codex_toml_field` / `write_codex_toml_field`）。

### 修改文件

**后端**：
- `src-tauri/src/commands/extra_commands.rs`
  - 新增 `read_codex_toml_structured` — 解析 config.toml 返回结构化 JSON
  - 新增 `write_codex_toml_structured` — 接收结构化 JSON 写回 TOML（保留注释和未知字段）
  - 字段：`model`、`base_url`、`api_key`、`model_context_window`、`model_auto_compact_token_limit`、`[mcp_servers]` 表

- `src-tauri/src/lib.rs` — 注册新命令

**前端**：
- `src/pages/ConfigFiles.tsx`
  - 在 Codex config.toml 文件上方新增结构化表单区域
  - 字段控件：Input（model、base_url、api_key）、Number（context_window）、开关（1M context）
  - `[mcp_servers]` 表：展示已配 MCP 列表，支持检测格式错误并提供自动修复按钮
  - 表单提交后同步更新下方 TOML 编辑器内容

---

## 3. 端点延迟测速

**目标**：Profiles 页 Provider 卡片支持端点延迟测量，显示响应时间和质量指示器。

**与健康检查的区别**：健康检查（Stream Check）发真实 API 请求验证可用性，延迟测速仅做轻量 HTTP HEAD/GET 测量网络延迟。

### 修改文件

**后端**：
- `src-tauri/src/commands/extra_commands.rs`
  - 新增 `ping_provider_endpoint` 命令
  - 向 Provider 的 base_url 发送 HEAD 请求（或 GET /），记录耗时
  - 支持代理（复用 `get_proxy`）
  - 返回 `{ latencyMs: number, status: "fast" | "medium" | "slow" | "error" }`
  - 阈值：< 200ms = fast，200-500ms = medium，> 500ms = slow

- `src-tauri/src/lib.rs` — 注册新命令

**前端**：
- `src/pages/Profiles.tsx`
  - Provider 卡片新增测速按钮（Activity 图标）
  - 点击后显示延迟 ms 数 + 颜色指示（绿/黄/红）
  - 结果缓存在组件 state 中，不持久化

---

## 4. 用量自动刷新

**目标**：Logs 页面支持自动轮询刷新代理请求日志。

### 修改文件

**前端**：
- `src/pages/Logs.tsx`
  - 新增自动刷新开关（Toggle + 间隔选择：5s / 10s / 30s / 60s）
  - 启用后通过 `setInterval` 定时调用现有数据获取函数
  - 切换页面或关闭开关时清理 interval
  - 刷新时不重置滚动位置，增量追加新数据

---

## 5. Claude Quick Toggles

**目标**：ConfigFiles 页在 Claude 配置 JSON 编辑器上方叠加常用设置的快捷开关。

### 修改文件

**后端**：
- `src-tauri/src/commands/extra_commands.rs`
  - 新增 `read_claude_config_toggles` — 读取 Claude settings.json 中的指定字段
  - 新增 `write_claude_config_toggle` — 修改单个字段并写回
  - 字段列表：`env.ANTHROPIC_HIDE_ATTRIBUTION`、`env.CLAUDE_CODE_ENABLE_TEAMMATES`、`env.CLAUDE_CODE_MAX_THINKING_TOKENS`、`env.ENABLE_TOOL_SEARCH`

- `src-tauri/src/lib.rs` — 注册新命令

**前端**：
- `src/pages/ConfigFiles.tsx`
  - 在 Claude settings.json 编辑器上方新增 Quick Toggles 栏
  - 每个 Toggle：标签 + Switch 组件
  - 切换时调用后端写入，并刷新编辑器内容
  - 样式：水平排列的 badge 式开关，紧凑不占空间

---

## 实现顺序

1. **第 4 项**（用量自动刷新）— 纯前端，最简单
2. **第 3 项**（端点延迟测速）— 后端简单 + 前端简单
3. **第 5 项**（Claude Quick Toggles）— 后端中等 + 前端中等
4. **第 2 项**（Codex TOML 结构化编辑）— 后端中等 + 前端中等
5. **第 1 项**（Common Config Snippet）— 最复杂，需要修改 apply 逻辑

## 验证

1. `cargo check` — 后端编译通过
2. `pnpm exec tsc -b` — 前端类型检查通过
3. `pnpm build` — 生产构建通过
