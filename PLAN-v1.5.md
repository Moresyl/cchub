# CCHub v1.5 功能扩展计划

## 目标

参考 cc-switch v3.14 的功能，为 cchub 添加以下核心功能模块。

---

## Phase 1: Hermes Agent 深度管理 ✅ 已完成

**现状**: 已有基础的 hermes 模块（config 读写、provider 预设、env、mcp、snapshot），但缺少完整的前端管理界面。

### 1.1 Hermes Memory 面板 ✅ 已完成
- [x] 前端: `src/pages/HermesMemory.tsx`
- [x] 支持编辑 `MEMORY.md` 和 `USER.md`（Tab 切换）
- [x] 启用/禁用开关
- [x] 字符数限制提示（超限红色警告）
- [x] 保存功能
- [x] Rust: `commands/hermes_commands.rs` — 4 个命令: get_limits, get_content, save_content, toggle_enabled
- [x] 侧边栏入口 + 路由 + i18n (中/英/日)

### 1.2 Hermes Provider 管理 ✅ 已完成
- [x] Rust: `commands/hermes_commands.rs` — 6 个 CRUD 命令 (list/get/save/delete/set_active/get_active)
- [x] 前端: `src/pages/HermesProviders.tsx` — Provider 列表 + 编辑表单
- [x] 支持 4 种 API 协议: `chat_completions`, `anthropic_messages`, `codex_responses`, `bedrock_converse`
- [x] 内置 6 个 preset 快速填充 (openrouter, anthropic, gemini, nous, zai, kimi-coding)
- [x] 活跃 Provider 切换（写入 config.yaml model 节）
- [x] 侧边栏入口 (Bot 图标) + 路由 + i18n (中/英/日)

### 1.3 Hermes MCP/Skills 同步 ✅ 已有
- [x] 复用已有的 `hermes::mcp` 模块 — sync/unsync/check 全部就位
- [x] 统一 MCP 管理面板已含 Hermes 同步按钮 (check_mcp_server_in_tools 含 "hermes")
- [x] 统一 Skills 管理已含 Hermes 同步 (copy_skill_between_tools 支持 hermes 目标)

---

## Phase 2: OpenClaw 深度集成 ✅ 已完成

**现状**: cchub 已支持 OpenClaw 的 MCP 同步和基础配置。

### 2.1 OpenClaw 统一管理面板 ✅
- [x] 前端: `src/pages/OpenClaw.tsx` — 三 Tab 合一面板 (Env / Tools / Agents)
- [x] Rust: `src-tauri/src/openclaw_config.rs` — 配置读写模块
- [x] Rust: `src-tauri/src/commands/openclaw_commands.rs` — 8 个 Tauri 命令
- [x] 环境变量 CRUD (key-value 编辑器)
- [x] 工具配置: profile 选择 (minimal/coding/messaging/full) + allow/deny 列表
- [x] Agents 默认模型: primary model + fallback models 编辑
- [x] 健康检查: 配置解析、缺失 baseUrl、空环境变量警告
- [x] 安装检测: 未安装时显示引导页
- [x] 侧边栏入口 (Terminal 图标) + 路由 + i18n (中/英/日)

---

## Phase 3: Model Auto-Fetch 增强 ✅ 已完成

**现状**: 已有 `provider_models.rs` 支持 OpenAI/Anthropic/Gemini 的 `/v1/models` API。

### 3.1 模型缓存 ✅
- [x] `fetch_provider_models_cached` — 带 10 分钟 TTL 缓存的模型获取
- [x] `get_cached_provider_models` — 读取缓存（无网络请求）
- [x] 缓存存储在 SQLite `app_settings` 表，key 为 tool_id+base_url 的 hash
- [x] 支持 `force_refresh` 参数强制刷新

### 3.2 前端集成 ✅
- [x] Provider 表单中已有"自动获取模型"按钮
- [x] 模型下拉框支持搜索过滤 (`ModelSelector` 组件 — 实时搜索 + 弹出列表)
- [x] 显示模型元信息 (context window / max output / pricing) — `fetch_provider_models_detailed` 命令

---

## Phase 4: 代理增强功能

**现状**: 已有 `provider_proxy.rs` 实现了基础的本地代理，支持 Anthropic↔OpenAI 格式转换。

### 4.1 Thinking Optimizer (思维优化器) ✅ 已完成
- [x] Rust: `src-tauri/src/proxy_optimizer/thinking_optimizer.rs`
- [x] 三路径调度: haiku→跳过, opus/sonnet→adaptive thinking, 其他→legacy budget
- [x] 自动注入 `anthropic_beta` headers (context-1m / interleaved-thinking)
- [x] 完整单元测试

### 4.2 Model Mapper (模型映射) ✅ 已完成
- [x] Rust: `src-tauri/src/proxy_optimizer/model_mapper.rs`
- [x] 支持环境变量驱动: ANTHROPIC_DEFAULT_HAIKU/SONNET/OPUS_MODEL
- [x] 自定义规则: Contains / Exact 匹配模式
- [x] 完整单元测试
- [x] 前端: 模型映射规则编辑界面（ProxyAdvanced 页面）
- [x] 集成到 `apply_proxy_optimizers` 管道

### 4.3 Cache Injector (缓存注入) ✅ 已完成
- [x] Rust: `src-tauri/src/proxy_optimizer/cache_injector.rs`
- [x] 自动注入最多 4 个 cache_control breakpoints (tools末尾、system末尾、assistant最后非thinking块)
- [x] 升级已有 TTL、string system → array 格式转换
- [x] 完整单元测试

### 4.4 Body Filter (请求体过滤) ✅ 已完成
- [x] Rust: `src-tauri/src/proxy_optimizer/body_filter.rs`
- [x] 递归移除 `_` 前缀私有字段
- [x] 支持白名单保留特定字段
- [x] 完整单元测试

### 4.5 代理增强集成 ✅ 已完成
- [x] `src-tauri/src/proxy_optimizer/mod.rs` — 模块根
- [x] `src-tauri/src/proxy_optimizer/config.rs` — OptimizerConfig (SQLite 持久化)
- [x] `commands/optimizer_commands.rs` — get/set 配置 Tauri 命令
- [x] `apply_proxy_optimizers()` 集成到 provider_proxy 管道
- [x] 管道顺序: body_filter → thinking_optimizer → cache_injector
- [x] 仅对 `tool_id == "claude"` 请求生效

### 4.6 代理增强配置面板 ✅ 已完成
- [x] 前端: `src/pages/ProxyAdvanced.tsx`
- [x] 总开关 + 各模块独立开关
- [x] Cache TTL 设置
- [x] Body Filter 白名单编辑器
- [x] 侧边栏入口 (Sparkles 图标) + 路由 + i18n (中/英/日)

### 4.7 Copilot Optimizer ✅ 已完成
- [x] Rust: `src-tauri/src/proxy_optimizer/copilot_optimizer.rs`
- [x] 请求分类: classify_request() — x-initiator (user/agent) 自动判定
- [x] Tool Result 合并: merge_tool_results() — 消息内部 + 跨消息合并
- [x] 孤立 Tool Result 清理: sanitize_orphan_tool_results()
- [x] Thinking Block 剥离: strip_thinking_blocks()
- [x] Compact 检测: is_compact_request() — 三种强特征信号
- [x] 子代理检测: detect_subagent() — __SUBAGENT_MARKER__ + metadata fallback
- [x] 确定性 ID: deterministic_request_id() + deterministic_interaction_id()
- [x] 集成到 apply_proxy_optimizers 管道（在其他优化器之前执行）
- [x] OptimizerConfig 新增 6 个 copilot 配置字段
- [x] 前端: ProxyAdvanced 页面新增 Copilot Optimizer 区域（主开关 + 5 个子开关）
- [x] i18n: 中/英/日三语支持
- [x] 完整单元测试（16 个 test case）

### 4.8 Gemini Native API 代理 ✅ 已完成
- [x] Rust: `src-tauri/src/gemini_transform.rs`
- [x] Anthropic → Gemini: messages/system/tools/generation_config 完整转换
- [x] Gemini → Anthropic: candidates/functionCall/usage 响应转换
- [x] ClaudeApiFormat 新增 `GeminiNative` 变体
- [x] URL 路由: `v1beta/models/{model}:generateContent` / `streamGenerateContent?alt=sse`
- [x] Provider 配置 `api_format: "gemini_native"` 即可启用
- [x] 工具调用: Anthropic tool_use/tool_result ↔ Gemini functionCall/functionResponse
- [x] 完整单元测试（7 个 test case）
- [x] Gemini SSE → Anthropic SSE 流式转换 (`create_anthropic_sse_stream_from_gemini` — 完整事件映射)

### 4.9 全局代理配置 ✅ 已有
- [x] 前端: Settings 中 `SettingsNetworkProxySection` 组件
- [x] 支持 HTTP/HTTPS/SOCKS5 代理
- [x] `app_settings.proxy_url` 存储
- [x] 所有出站请求统一走配置的代理

---

## Phase 5: 导航 & UI 整合 ✅ 已完成

### 5.1 侧边栏 App 切换 ✅
- [x] 侧边栏增加 Hermes Memory 入口 (Brain 图标)
- [x] 侧边栏增加 OpenClaw 入口 (Terminal 图标)
- [x] 侧边栏增加代理增强入口 (Sparkles 图标)

### 5.2 代理配置面板 ✅
- [x] 前端: `src/pages/ProxyAdvanced.tsx`
- [x] 展示所有代理增强模块的开关和配置
- [x] Model Mapper 规则编辑器
- [x] Thinking Optimizer 开关
- [x] Body Filter 白名单编辑器

---

## 实施进度

| 阶段 | 状态 | 备注 |
|------|------|------|
| Phase 4.1-4.4 (代理核心增强) | ✅ 完成 | Thinking/Cache/Body/ModelMapper 四模块 |
| Phase 4.5 (集成 + 配置命令) | ✅ 完成 | 管道集成 + Tauri 命令 |
| Phase 1.1 (Hermes Memory) | ✅ 完成 | 前后端 + i18n |
| Phase 2 (OpenClaw 深度集成) | ✅ 完成 | 统一三 Tab 面板 |
| Phase 3 (Model Auto-Fetch) | ✅ 完成 | 10 分钟 TTL 缓存层 |
| Phase 4.6 (配置面板) | ✅ 完成 | ProxyAdvanced 页面 |
| Phase 5 (UI 整合) | ✅ 完成 | 侧边栏 + 路由 |
| Phase 4.7 (Copilot Optimizer) | ✅ 完成 | 请求分类 + 合并 + 清理 + 剥离 + 检测 |
| Phase 1.2 (Hermes Provider) | ✅ 完成 | CRUD + 活跃切换 + Preset |
| Phase 1.3 (Hermes MCP/Skills 同步) | ✅ 完成 | 已有基础设施全面支持 |
| Phase 4.8 (Gemini Native API) | ✅ 完成 | 格式转换 + 路由 (流式 pass-through) |

---

## 技术约束

- Tauri v2 + React + TypeScript + Rust
- 严格使用项目现有 CSS 体系（CSS 变量 + 项目 class + inline styles）
- 中文 commit 信息
- 版本号同步: package.json / tauri.conf.json / Cargo.toml

---

## 新增文件清单

```
src-tauri/src/proxy_optimizer/
├── mod.rs              # 模块根
├── config.rs           # OptimizerConfig 结构体 + SQLite 持久化
├── thinking_optimizer.rs  # Thinking 三路径调度
├── cache_injector.rs   # 自动缓存断点注入
├── body_filter.rs      # _ 前缀字段过滤
├── model_mapper.rs     # 模型映射
└── copilot_optimizer.rs # Copilot 请求分类 + 合并 + 清理

src-tauri/src/openclaw_config.rs      # OpenClaw 配置读写
src-tauri/src/gemini_transform.rs    # Gemini Native API 格式转换
src-tauri/src/commands/hermes_commands.rs    # Hermes Memory + Provider 命令
src-tauri/src/commands/openclaw_commands.rs  # OpenClaw 管理命令
src-tauri/src/commands/optimizer_commands.rs # 优化器配置命令

src/pages/HermesMemory.tsx    # Hermes 记忆面板
src/pages/HermesProviders.tsx # Hermes 供应商管理面板
src/pages/OpenClaw.tsx        # OpenClaw 管理面板
src/pages/ProxyAdvanced.tsx   # 代理增强配置面板
```
