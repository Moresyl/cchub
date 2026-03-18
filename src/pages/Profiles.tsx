import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw, Save, Trash2, Check, X, ArrowRightLeft,
  Search, Terminal, Code, Monitor, Sparkles, Globe,
  Edit3, Plus, Eye, EyeOff, Copy, Cat,
} from "lucide-react";
import { getLocale } from "../lib/i18n";
import {
  applyPresetToFields,
  buildStructuredConfig,
  createDefaultStructuredFields,
  getPresetCategories,
  parseStructuredConfig,
  supportsStructuredConfig,
  type ApiFormat,
  type ClaudeAuthField,
} from "../lib/configProfiles";
import { showToast } from "../components/Toast";
import CodeEditor from "../components/CodeEditor";
import ConfirmDialog from "../components/ConfirmDialog";

interface ConfigProfile {
  id: string;
  name: string;
  tool_id: string;
  config_snapshot: string;
  source_type?: string | null;
  source_key?: string | null;
  created_at: string | null;
  updated_at: string | null;
}

interface DetectedTool {
  id: string;
  name: string;
  installed: boolean;
}

const TOOL_ICONS: Record<string, typeof Monitor> = {
  claude: Terminal,
  codex: Code,
  gemini: Sparkles,
  opencode: Globe,
  openclaw: Cat,
};

function formatTime(value: string | null) {
  if (!value) return "";
  return value.replace("T", " ").slice(0, 19);
}

function prettyJson(content: string): string {
  try {
    return JSON.stringify(JSON.parse(content), null, 2);
  } catch {
    return content;
  }
}

function getConfigLanguage(toolId: string, content: string): "json" | "toml" {
  if (toolId === "codex") {
    try {
      JSON.parse(content);
      return "json";
    } catch {
      return "toml";
    }
  }
  return "json";
}

/** 从配置快照中提取摘要信息用于卡片展示 */
function extractConfigSummary(toolId: string, content: string): { baseUrl?: string; model?: string } {
  try {
    const parsed = JSON.parse(content) as Record<string, any>;
    if (toolId === "claude") {
      const env = (parsed.env || {}) as Record<string, string>;
      return {
        baseUrl: env.ANTHROPIC_BASE_URL,
        model: env.ANTHROPIC_MODEL || env.ANTHROPIC_DEFAULT_SONNET_MODEL,
      };
    }
    if (toolId === "gemini") {
      const env = (parsed.env || {}) as Record<string, string>;
      return {
        baseUrl: env.GOOGLE_GEMINI_BASE_URL,
        model: env.GEMINI_MODEL,
      };
    }
    if (toolId === "codex") {
      const config = typeof parsed.config === "string" ? parsed.config : "";
      const modelMatch = config.match(/^model\s*=\s*"([^"]*)"/m);
      const urlMatch = config.match(/^base_url\s*=\s*"([^"]*)"/m);
      return {
        baseUrl: urlMatch?.[1],
        model: modelMatch?.[1],
      };
    }
  } catch { /* ignore */ }
  return {};
}

/** Codex 配置分拆显示：auth.json + config.toml */
function CodexRawConfigEditor({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  let authJson = "";
  let configToml = "";
  try {
    const parsed = JSON.parse(value) as Record<string, any>;
    authJson = JSON.stringify(parsed.auth || {}, null, 2);
    configToml = typeof parsed.config === "string" ? parsed.config : "";
  } catch {
    // fallback: show raw
    return <CodeEditor value={value} onChange={onChange} language="json" minHeight={240} />;
  }

  function rebuild(nextAuth: string, nextToml: string) {
    try {
      const auth = JSON.parse(nextAuth);
      onChange(JSON.stringify({ auth, config: nextToml }, null, 2));
    } catch { /* ignore invalid JSON */ }
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <label className="field-label" style={{ marginBottom: 6 }}>auth.json</label>
        <CodeEditor
          value={authJson}
          onChange={(v) => rebuild(v, configToml)}
          language="json"
          minHeight={80}
        />
      </div>
      <div>
        <label className="field-label" style={{ marginBottom: 6 }}>config.toml</label>
        <CodeEditor
          value={configToml}
          onChange={(v) => rebuild(authJson, v)}
          language="toml"
          minHeight={200}
        />
      </div>
    </div>
  );
}

export default function Profiles() {
  const [profiles, setProfiles] = useState<ConfigProfile[]>([]);
  const [tools, setTools] = useState<DetectedTool[]>([]);
  const [activeIds, setActiveIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [newTool, setNewTool] = useState("claude");
  const [saving, setSaving] = useState(false);
  const [applying, setApplying] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingProfile, setEditingProfile] = useState<ConfigProfile | null>(null);
  const [draftName, setDraftName] = useState("");
  const [draftTool, setDraftTool] = useState("claude");
  const [draftContent, setDraftContent] = useState("");
  const [draftLoading, setDraftLoading] = useState(false);
  const [draftPresetId, setDraftPresetId] = useState("custom");
  const [draftBaseUrl, setDraftBaseUrl] = useState("");
  const [draftApiKey, setDraftApiKey] = useState("");
  const [draftModel, setDraftModel] = useState("");
  const [draftReasoningModel, setDraftReasoningModel] = useState("");
  const [draftHaikuModel, setDraftHaikuModel] = useState("");
  const [draftSonnetModel, setDraftSonnetModel] = useState("");
  const [draftOpusModel, setDraftOpusModel] = useState("");
  const [draftAuthField, setDraftAuthField] = useState<ClaudeAuthField>("ANTHROPIC_AUTH_TOKEN");
  const [draftApiFormat, setDraftApiFormat] = useState<ApiFormat>("anthropic");
  const [filterTool, setFilterTool] = useState("claude");
  const [search, setSearch] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);
  const [confirmAction, setConfirmAction] = useState<{ type: string; profile: ConfigProfile } | null>(null);
  const locale = getLocale();

  useEffect(() => { void load(); }, []);

  async function load() {
    setLoading(true);
    try {
      await invoke("sync_config_profiles");
      const [nextProfiles, nextTools, nextActiveIds] = await Promise.all([
        invoke<ConfigProfile[]>("get_config_profiles"),
        invoke<DetectedTool[]>("detect_tools"),
        invoke<string[]>("get_active_config_profile_ids"),
      ]);
      setProfiles(nextProfiles);
      setTools(nextTools);
      setActiveIds(nextActiveIds);
      setNewTool((prev) => {
        const installed = nextTools.filter((tool) => tool.installed);
        if (installed.some((tool) => tool.id === prev)) return prev;
        return installed[0]?.id || nextTools[0]?.id || "claude";
      });
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `加载失败: ${e}` : `Load failed: ${e}`);
    } finally {
      setLoading(false);
    }
  }

  function updateStructuredDraft(toolId: string, next: {
    presetId?: string;
    baseUrl?: string;
    apiKey?: string;
    model?: string;
    reasoningModel?: string;
    haikuModel?: string;
    sonnetModel?: string;
    opusModel?: string;
    authField?: ClaudeAuthField;
    apiFormat?: ApiFormat;
  }) {
    const fields = {
      presetId: next.presetId ?? draftPresetId,
      baseUrl: next.baseUrl ?? draftBaseUrl,
      apiKey: next.apiKey ?? draftApiKey,
      model: next.model ?? draftModel,
      reasoningModel: next.reasoningModel ?? draftReasoningModel,
      haikuModel: next.haikuModel ?? draftHaikuModel,
      sonnetModel: next.sonnetModel ?? draftSonnetModel,
      opusModel: next.opusModel ?? draftOpusModel,
      authField: next.authField ?? draftAuthField,
      apiFormat: next.apiFormat ?? draftApiFormat,
    };
    setDraftPresetId(fields.presetId);
    setDraftBaseUrl(fields.baseUrl);
    setDraftApiKey(fields.apiKey);
    setDraftModel(fields.model);
    setDraftReasoningModel(fields.reasoningModel);
    setDraftHaikuModel(fields.haikuModel);
    setDraftSonnetModel(fields.sonnetModel);
    setDraftOpusModel(fields.opusModel);
    setDraftAuthField(fields.authField);
    setDraftApiFormat(fields.apiFormat);
    setDraftContent(buildStructuredConfig(toolId, fields));
  }

  async function openCreateModal(toolId?: string) {
    const selectedTool = toolId || newTool;
    if (!installedTools.length) {
      showToast("error", locale === "zh" ? "没有可用工具配置" : "No available tool configuration");
      return;
    }
    setEditingProfile(null);
    setDraftName("");
    setDraftTool(selectedTool);
    setDraftContent("");
    setShowCreateModal(true);
    setSaving(false);
    setNewTool(selectedTool);
    setShowApiKey(false);
    setDraftApiFormat("anthropic");
    if (supportsStructuredConfig(selectedTool)) {
      const defaults = createDefaultStructuredFields(selectedTool);
      setDraftPresetId(defaults.presetId);
      setDraftBaseUrl(defaults.baseUrl);
      setDraftApiKey(defaults.apiKey);
      setDraftModel(defaults.model);
      setDraftReasoningModel(defaults.reasoningModel);
      setDraftHaikuModel(defaults.haikuModel);
      setDraftSonnetModel(defaults.sonnetModel);
      setDraftOpusModel(defaults.opusModel);
      setDraftAuthField(defaults.authField);
      setDraftContent(buildStructuredConfig(selectedTool, defaults));
      setDraftLoading(false);
      return;
    }
    setDraftPresetId("custom");
    setDraftBaseUrl("");
    setDraftApiKey("");
    setDraftModel("");
    setDraftReasoningModel("");
    setDraftHaikuModel("");
    setDraftSonnetModel("");
    setDraftOpusModel("");
    setDraftAuthField("ANTHROPIC_AUTH_TOKEN");
    setDraftLoading(true);
    try {
      const configContent = await invoke<string>("read_tool_config", { toolId: selectedTool });
      setDraftContent(prettyJson(configContent));
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `读取配置失败: ${e}` : `Failed to read configuration: ${e}`);
    } finally {
      setDraftLoading(false);
    }
  }

  function openEditModal(profile: ConfigProfile) {
    setEditingProfile(profile);
    setShowCreateModal(false);
    setDraftName(profile.name);
    setDraftTool(profile.tool_id);
    setDraftContent(prettyJson(profile.config_snapshot));
    setShowApiKey(false);
    if (supportsStructuredConfig(profile.tool_id)) {
      const parsed = parseStructuredConfig(profile.tool_id, profile.config_snapshot);
      setDraftPresetId(parsed.presetId);
      setDraftBaseUrl(parsed.baseUrl);
      setDraftApiKey(parsed.apiKey);
      setDraftModel(parsed.model);
      setDraftReasoningModel(parsed.reasoningModel);
      setDraftHaikuModel(parsed.haikuModel);
      setDraftSonnetModel(parsed.sonnetModel);
      setDraftOpusModel(parsed.opusModel);
      setDraftAuthField(parsed.authField);
      setDraftApiFormat(parsed.apiFormat);
    } else {
      setDraftPresetId("custom");
      setDraftBaseUrl("");
      setDraftApiKey("");
      setDraftModel("");
      setDraftReasoningModel("");
      setDraftHaikuModel("");
      setDraftSonnetModel("");
      setDraftOpusModel("");
      setDraftAuthField("ANTHROPIC_AUTH_TOKEN");
      setDraftApiFormat("anthropic");
    }
    setDraftLoading(false);
  }

  function closeModal() {
    setShowCreateModal(false);
    setEditingProfile(null);
    setDraftName("");
    setDraftContent("");
    setDraftLoading(false);
    setDraftPresetId("custom");
    setDraftBaseUrl("");
    setDraftApiKey("");
    setDraftModel("");
    setDraftReasoningModel("");
    setDraftHaikuModel("");
    setDraftSonnetModel("");
    setDraftOpusModel("");
    setDraftAuthField("ANTHROPIC_AUTH_TOKEN");
    setDraftApiFormat("anthropic");
    setSaving(false);
    setShowApiKey(false);
  }

  async function handleSaveModal() {
    if (!draftName.trim() || saving) return;
    setSaving(true);
    try {
      if (editingProfile) {
        await invoke("update_config_profile", {
          id: editingProfile.id,
          name: draftName.trim(),
          configSnapshot: draftContent,
        });
        showToast("success", locale === "zh" ? "配置已更新" : "Configuration updated");
      } else {
        await invoke("save_config_profile", {
          name: draftName.trim(),
          toolId: draftTool,
          configSnapshot: draftContent,
        });
        showToast("success", locale === "zh" ? "配置已保存" : "Configuration saved");
      }
      closeModal();
      await load();
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `保存失败: ${e}` : `Save failed: ${e}`);
    } finally {
      setSaving(false);
    }
  }

  async function handleApply(profile: ConfigProfile) {
    void doApply(profile);
  }

  async function doApply(profile: ConfigProfile) {
    setApplying(profile.id);
    try {
      await invoke("apply_config_profile", { id: profile.id });
      await load();
      showToast("success", locale === "zh" ? "配置已切换" : "Configuration switched");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `切换失败: ${e}` : `Switch failed: ${e}`);
    } finally {
      setApplying(null);
    }
  }

  async function handleDelete(profile: ConfigProfile) {
    setConfirmAction({ type: "delete", profile });
  }

  async function doDelete(profile: ConfigProfile) {
    try {
      await invoke("delete_config_profile", { id: profile.id });
      await load();
      showToast("success", locale === "zh" ? "配置已删除" : "Configuration deleted");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `删除失败: ${e}` : `Delete failed: ${e}`);
    }
  }

  async function handleDuplicate(profile: ConfigProfile) {
    try {
      const name = profile.name + (locale === "zh" ? " (副本)" : " (Copy)");
      await invoke("save_config_profile", {
        name,
        toolId: profile.tool_id,
        configSnapshot: profile.config_snapshot,
      });
      await load();
      showToast("success", locale === "zh" ? "配置已复制" : "Configuration duplicated");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `复制失败: ${e}` : `Duplicate failed: ${e}`);
    }
  }

  const activeIdSet = useMemo(() => new Set(activeIds), [activeIds]);
  const installedTools = useMemo(() => tools.filter((tool) => tool.installed), [tools]);
  const presetCategories = useMemo(() => getPresetCategories(draftTool), [draftTool]);

  const toolCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const profile of profiles) {
      counts[profile.tool_id] = (counts[profile.tool_id] || 0) + 1;
    }
    return counts;
  }, [profiles]);

  const filteredProfiles = useMemo(() => {
    const keyword = search.trim().toLowerCase();
    return [...profiles]
      .filter((profile) => {
        if (filterTool && profile.tool_id !== filterTool) return false;
        if (!keyword) return true;
        return (
          profile.name.toLowerCase().includes(keyword) ||
          profile.tool_id.toLowerCase().includes(keyword) ||
          profile.config_snapshot.toLowerCase().includes(keyword)
        );
      })
      .sort((a, b) => {
        const activeDiff = Number(activeIdSet.has(b.id)) - Number(activeIdSet.has(a.id));
        if (activeDiff !== 0) return activeDiff;
        const aTime = a.updated_at || a.created_at || "";
        const bTime = b.updated_at || b.created_at || "";
        return bTime.localeCompare(aTime);
      });
  }, [profiles, filterTool, search, activeIdSet]);

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
          {locale === "zh" ? "加载中..." : "Loading..."}
        </span>
      </div>
    );
  }

  const isEditing = showCreateModal || !!editingProfile;
  const isStructured = supportsStructuredConfig(draftTool);

  // --- 编辑 / 新增视图 ---
  if (isEditing) {
    return (
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <div className="page-header">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <button className="btn btn-ghost btn-icon-sm" onClick={closeModal} title={locale === "zh" ? "返回" : "Back"}>
              <X size={18} />
            </button>
            <div>
              <h2 className="page-title">
                {editingProfile
                  ? (locale === "zh" ? "编辑配置" : "Edit Configuration")
                  : (locale === "zh" ? "新增配置" : "New Configuration")}
              </h2>
              <p className="page-subtitle">
                {editingProfile
                  ? (locale === "zh" ? "修改配置名称和参数" : "Update configuration name and parameters")
                  : (locale === "zh" ? "创建一个新的工具配置" : "Create a new tool configuration")}
              </p>
            </div>
          </div>
        </div>

        <div style={{ flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 20, paddingBottom: 20 }}>
          {/* 基本信息 */}
          <div>
            <h3 style={{ fontSize: 13, fontWeight: 600, color: "var(--text-muted)", marginBottom: 12, textTransform: "uppercase", letterSpacing: "0.05em" }}>
              {locale === "zh" ? "基本信息" : "Basic Info"}
            </h3>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <label className="field-label">{locale === "zh" ? "工具" : "Tool"}</label>
                <select
                  className="input"
                  value={draftTool}
                  disabled={!!editingProfile}
                  onChange={async (e) => {
                    const toolId = e.target.value;
                    setDraftTool(toolId);
                    setNewTool(toolId);
                    setDraftApiFormat("anthropic");
                    if (supportsStructuredConfig(toolId)) {
                      const defaults = createDefaultStructuredFields(toolId);
                      setDraftPresetId(defaults.presetId);
                      setDraftBaseUrl(defaults.baseUrl);
                      setDraftApiKey(defaults.apiKey);
                      setDraftModel(defaults.model);
                      setDraftReasoningModel(defaults.reasoningModel);
                      setDraftHaikuModel(defaults.haikuModel);
                      setDraftSonnetModel(defaults.sonnetModel);
                      setDraftOpusModel(defaults.opusModel);
                      setDraftAuthField(defaults.authField);
                      setDraftContent(buildStructuredConfig(toolId, defaults));
                      setDraftLoading(false);
                    } else {
                      setDraftPresetId("custom");
                      setDraftBaseUrl("");
                      setDraftApiKey("");
                      setDraftModel("");
                      setDraftReasoningModel("");
                      setDraftHaikuModel("");
                      setDraftSonnetModel("");
                      setDraftOpusModel("");
                      setDraftAuthField("ANTHROPIC_AUTH_TOKEN");
                      setDraftContent("");
                      setDraftLoading(true);
                      try {
                        const configContent = await invoke<string>("read_tool_config", { toolId });
                        setDraftContent(prettyJson(configContent));
                      } catch (error) {
                        console.error(error);
                      } finally {
                        setDraftLoading(false);
                      }
                    }
                  }}
                  style={{ fontSize: 13 }}
                >
                  {tools.map((tool) => (
                    <option key={tool.id} value={tool.id}>{tool.name}</option>
                  ))}
                </select>
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <label className="field-label">{locale === "zh" ? "配置名称" : "Name"}</label>
                <input
                  className="input"
                  placeholder={locale === "zh" ? "例如：官方 API、中转服务" : "e.g. Official API, Proxy Service"}
                  value={draftName}
                  onChange={(e) => setDraftName(e.target.value)}
                  style={{ fontSize: 13 }}
                  autoFocus
                />
              </div>
            </div>
          </div>

          {/* 结构化表单 */}
          {isStructured && (
            <>
              {/* 预设选择 */}
              <div>
                <h3 style={{ fontSize: 13, fontWeight: 600, color: "var(--text-muted)", marginBottom: 12, textTransform: "uppercase", letterSpacing: "0.05em" }}>
                  {locale === "zh" ? "预设模板" : "Preset"}
                </h3>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                  {presetCategories.flatMap((group) =>
                    group.presets.map((preset) => (
                      <button
                        key={preset.id}
                        className={`btn btn-sm ${draftPresetId === preset.id ? "btn-primary" : "btn-secondary"}`}
                        onClick={() => {
                          const next = applyPresetToFields(draftTool, preset.id, {
                            apiKey: draftApiKey,
                            authField: draftAuthField,
                            apiFormat: draftApiFormat,
                          });
                          updateStructuredDraft(draftTool, next);
                        }}
                      >
                        {preset.name}
                      </button>
                    ))
                  )}
                </div>
              </div>

              {/* 连接配置 */}
              <div>
                <h3 style={{ fontSize: 13, fontWeight: 600, color: "var(--text-muted)", marginBottom: 12, textTransform: "uppercase", letterSpacing: "0.05em" }}>
                  {locale === "zh" ? "连接配置" : "Connection"}
                </h3>
                <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
                  <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <label className="field-label">API Key</label>
                    <div style={{ position: "relative" }}>
                      <input
                        className="input"
                        type={showApiKey ? "text" : "password"}
                        value={draftApiKey}
                        onChange={(e) => updateStructuredDraft(draftTool, { apiKey: e.target.value })}
                        placeholder={locale === "zh" ? "填写 API Key" : "Enter API Key"}
                        style={{ fontSize: 13, paddingRight: 40 }}
                      />
                      <button
                        className="btn btn-ghost btn-icon-sm"
                        style={{ position: "absolute", right: 4, top: "50%", transform: "translateY(-50%)" }}
                        onClick={() => setShowApiKey(!showApiKey)}
                        type="button"
                      >
                        {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
                      </button>
                    </div>
                  </div>

                  <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <label className="field-label">{locale === "zh" ? "接口地址" : "Base URL"}</label>
                    <input
                      className="input"
                      value={draftBaseUrl}
                      onChange={(e) => updateStructuredDraft(draftTool, { baseUrl: e.target.value })}
                      placeholder="https://api.example.com"
                      style={{ fontSize: 13 }}
                    />
                  </div>

                  {draftTool === "claude" && (
                    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
                      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                        <label className="field-label">{locale === "zh" ? "认证字段" : "Auth Field"}</label>
                        <select
                          className="input"
                          value={draftAuthField}
                          onChange={(e) => updateStructuredDraft(draftTool, { authField: e.target.value as ClaudeAuthField })}
                          style={{ fontSize: 13 }}
                        >
                          <option value="ANTHROPIC_AUTH_TOKEN">ANTHROPIC_AUTH_TOKEN</option>
                          <option value="ANTHROPIC_API_KEY">ANTHROPIC_API_KEY</option>
                        </select>
                      </div>
                      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                        <label className="field-label">{locale === "zh" ? "API 格式" : "API Format"}</label>
                        <select
                          className="input"
                          value={draftApiFormat}
                          onChange={(e) => updateStructuredDraft(draftTool, { apiFormat: e.target.value as ApiFormat })}
                          style={{ fontSize: 13 }}
                        >
                          <option value="anthropic">Anthropic Messages</option>
                          <option value="openai_chat">OpenAI Chat Completions</option>
                          <option value="openai_responses">OpenAI Responses API</option>
                        </select>
                      </div>
                    </div>
                  )}
                </div>
              </div>

              {/* 模型配置 */}
              <div>
                <h3 style={{ fontSize: 13, fontWeight: 600, color: "var(--text-muted)", marginBottom: 12, textTransform: "uppercase", letterSpacing: "0.05em" }}>
                  {locale === "zh" ? "模型配置" : "Models"}
                </h3>
                {draftTool === "claude" ? (
                  <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 }}>
                    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                      <label className="field-label">{locale === "zh" ? "主模型" : "Main Model"}</label>
                      <input
                        className="input"
                        value={draftModel}
                        onChange={(e) => updateStructuredDraft(draftTool, { model: e.target.value })}
                        placeholder="claude-sonnet-4-5"
                        style={{ fontSize: 13 }}
                      />
                    </div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                      <label className="field-label">{locale === "zh" ? "推理模型" : "Reasoning Model"}</label>
                      <input
                        className="input"
                        value={draftReasoningModel}
                        onChange={(e) => updateStructuredDraft(draftTool, { reasoningModel: e.target.value })}
                        placeholder="claude-sonnet-4-5"
                        style={{ fontSize: 13 }}
                      />
                    </div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                      <label className="field-label">{locale === "zh" ? "Haiku 默认模型" : "Default Haiku"}</label>
                      <input
                        className="input"
                        value={draftHaikuModel}
                        onChange={(e) => updateStructuredDraft(draftTool, { haikuModel: e.target.value })}
                        placeholder="claude-haiku-3-5"
                        style={{ fontSize: 13 }}
                      />
                    </div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                      <label className="field-label">{locale === "zh" ? "Sonnet 默认模型" : "Default Sonnet"}</label>
                      <input
                        className="input"
                        value={draftSonnetModel}
                        onChange={(e) => updateStructuredDraft(draftTool, { sonnetModel: e.target.value })}
                        placeholder="claude-sonnet-4-5"
                        style={{ fontSize: 13 }}
                      />
                    </div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                      <label className="field-label">{locale === "zh" ? "Opus 默认模型" : "Default Opus"}</label>
                      <input
                        className="input"
                        value={draftOpusModel}
                        onChange={(e) => updateStructuredDraft(draftTool, { opusModel: e.target.value })}
                        placeholder="claude-opus-4-5"
                        style={{ fontSize: 13 }}
                      />
                    </div>
                  </div>
                ) : (
                  <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <label className="field-label">{locale === "zh" ? "模型" : "Model"}</label>
                    <input
                      className="input"
                      value={draftModel}
                      onChange={(e) => updateStructuredDraft(draftTool, { model: e.target.value })}
                      placeholder={locale === "zh" ? "模型名称" : "Model name"}
                      style={{ fontSize: 13 }}
                    />
                  </div>
                )}
              </div>

              {/* 原始配置 */}
              <div>
                <h3 style={{ fontSize: 13, fontWeight: 600, color: "var(--text-muted)", marginBottom: 12, textTransform: "uppercase", letterSpacing: "0.05em" }}>
                  {locale === "zh" ? "原始配置" : "Raw Configuration"}
                </h3>
                <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
                  {locale === "zh"
                    ? "上方表单字段会自动同步到此处，你也可以直接编辑原始配置。"
                    : "Form fields above are synced here. You can also edit the raw config directly."}
                </div>
                {draftTool === "codex" ? (
                  <CodexRawConfigEditor value={draftContent} onChange={setDraftContent} />
                ) : (
                  <CodeEditor
                    value={draftContent}
                    onChange={setDraftContent}
                    language={getConfigLanguage(draftTool, draftContent)}
                    minHeight={240}
                  />
                )}
              </div>
            </>
          )}

          {/* 非结构化工具：直接显示编辑器 */}
          {!isStructured && (
            <div>
              <h3 style={{ fontSize: 13, fontWeight: 600, color: "var(--text-muted)", marginBottom: 12, textTransform: "uppercase", letterSpacing: "0.05em" }}>
                {locale === "zh" ? "配置内容" : "Configuration"}
              </h3>
              <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
                {locale === "zh" ? "直接编辑完整配置内容。" : "Edit the full configuration directly."}
              </div>
              {draftLoading ? (
                <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: 200 }}>
                  <div className="spinner" />
                </div>
              ) : (
                <CodeEditor
                  value={draftContent}
                  onChange={setDraftContent}
                  language={getConfigLanguage(draftTool, draftContent)}
                  minHeight={300}
                />
              )}
            </div>
          )}
        </div>

        <div className="sticky-footer" style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={closeModal}>
            {locale === "zh" ? "取消" : "Cancel"}
          </button>
          <button
            className="btn btn-primary btn-sm"
            onClick={() => void handleSaveModal()}
            disabled={!draftName.trim() || saving}
            style={{ gap: 6 }}
          >
            {saving ? <div className="spinner" style={{ width: 14, height: 14 }} /> : <Save size={14} />}
            {locale === "zh" ? "保存" : "Save"}
          </button>
        </div>
      </div>
    );
  }

  // --- 列表视图 ---
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{locale === "zh" ? "配置切换" : "Config Profiles"}</h2>
          <p className="page-subtitle">
            {locale === "zh"
              ? `共 ${profiles.length} 个配置，当前生效 ${activeIds.length} 个`
              : `${profiles.length} profiles, ${activeIds.length} active`}
          </p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={() => void load()} style={{ gap: 6 }}>
            <RefreshCw size={14} />
            {locale === "zh" ? "刷新" : "Refresh"}
          </button>
          <button
            className="btn btn-primary btn-sm"
            onClick={() => void openCreateModal()}
            disabled={installedTools.length === 0}
            style={{ gap: 6 }}
          >
            <Plus size={14} />
            {locale === "zh" ? "新增" : "New"}
          </button>
        </div>
      </div>

      <div style={{ display: "flex", gap: 12, marginBottom: 16, alignItems: "center", flexWrap: "wrap" }}>
        <div style={{ position: "relative", flex: 1, minWidth: 240, maxWidth: 360 }}>
          <Search
            size={14}
            style={{ position: "absolute", left: 12, top: "50%", transform: "translateY(-50%)", color: "var(--text-muted)" }}
          />
          <input
            className="input"
            style={{ paddingLeft: 36 }}
            placeholder={locale === "zh" ? "搜索配置..." : "Search..."}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          {search && (
            <button
              className="btn btn-ghost btn-icon-sm"
              style={{ position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)" }}
              onClick={() => setSearch("")}
            >
              <X size={14} />
            </button>
          )}
        </div>
        <div className="tab-bar" style={{ overflow: "auto", flexShrink: 0 }}>
          {tools.map((tool) => (
            <button
              key={tool.id}
              className={`tab-item ${filterTool === tool.id ? "active" : ""}`}
              onClick={() => setFilterTool((prev) => prev === tool.id ? "" : tool.id)}
              style={{ opacity: tool.installed || (toolCounts[tool.id] || 0) > 0 ? 1 : 0.55 }}
            >
              {tool.name} ({toolCounts[tool.id] || 0})
            </button>
          ))}
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 10 }}>
        {filteredProfiles.length === 0 ? (
          <div className="card empty-state" style={{ flex: 1 }}>
            <div className="empty-icon">
              <ArrowRightLeft size={28} style={{ color: "var(--text-muted)" }} />
            </div>
            <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>
              {locale === "zh" ? "没有可显示的配置" : "No configurations to display"}
            </p>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8, maxWidth: 320 }}>
              {locale === "zh"
                ? "点击右上角「新增」保存一份当前配置，之后就可以在这里一键切换。"
                : "Click \"New\" to save a configuration, then switch here."}
            </p>
          </div>
        ) : (
          filteredProfiles.map((profile) => {
            const Icon = TOOL_ICONS[profile.tool_id] || Monitor;
            const isActive = activeIdSet.has(profile.id);
            const summary = extractConfigSummary(profile.tool_id, profile.config_snapshot);
            return (
              <div
                key={profile.id}
                className="card card-hover"
                style={{
                  padding: "16px 18px",
                  borderColor: isActive ? "var(--success)" : undefined,
                  boxShadow: isActive ? "0 0 0 1px color-mix(in srgb, var(--success) 30%, transparent)" : undefined,
                }}
              >
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
                  <div style={{ display: "flex", gap: 12, minWidth: 0, flex: 1, alignItems: "center" }}>
                    <div className="icon-box" style={{ width: 36, height: 36, borderRadius: 8, flexShrink: 0 }}>
                      <Icon size={16} />
                    </div>
                    <div style={{ minWidth: 0, flex: 1 }}>
                      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                        <span style={{ fontSize: 14, fontWeight: 600 }}>{profile.name}</span>
                        <span className="badge badge-muted" style={{ textTransform: "capitalize", fontSize: 10 }}>
                          {profile.tool_id}
                        </span>
                        {isActive && (
                          <span className="badge badge-success" style={{ fontSize: 10 }}>
                            {locale === "zh" ? "当前生效" : "Active"}
                          </span>
                        )}
                      </div>
                      <div style={{ display: "flex", gap: 16, marginTop: 4, fontSize: 12, color: "var(--text-muted)" }}>
                        {summary.baseUrl && (
                          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 260 }}>
                            {summary.baseUrl}
                          </span>
                        )}
                        {summary.model && (
                          <span style={{ flexShrink: 0 }}>{summary.model}</span>
                        )}
                        {!summary.baseUrl && !summary.model && (
                          <span>{formatTime(profile.updated_at || profile.created_at)}</span>
                        )}
                      </div>
                    </div>
                  </div>

                  <div className="card-actions" style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
                    <button
                      className={`btn btn-xs ${isActive ? "btn-secondary" : "btn-primary"}`}
                      onClick={() => void handleApply(profile)}
                      disabled={applying === profile.id}
                      style={{ gap: 5 }}
                    >
                      {applying === profile.id ? (
                        <div className="spinner" style={{ width: 11, height: 11 }} />
                      ) : isActive ? (
                        <Check size={11} />
                      ) : (
                        <ArrowRightLeft size={11} />
                      )}
                      {locale === "zh" ? (isActive ? "已生效" : "切换") : (isActive ? "Active" : "Apply")}
                    </button>
                    <button
                      className="btn btn-ghost btn-icon-sm"
                      onClick={() => void handleDuplicate(profile)}
                      title={locale === "zh" ? "复制" : "Duplicate"}
                    >
                      <Copy size={14} />
                    </button>
                    <button
                      className="btn btn-ghost btn-icon-sm"
                      onClick={() => openEditModal(profile)}
                      title={locale === "zh" ? "编辑" : "Edit"}
                    >
                      <Edit3 size={14} />
                    </button>
                    <button
                      className="btn btn-danger-ghost btn-icon-sm"
                      onClick={() => void handleDelete(profile)}
                      title={locale === "zh" ? "删除" : "Delete"}
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>

      <ConfirmDialog
        isOpen={!!confirmAction}
        title={locale === "zh" ? "删除配置" : "Delete Configuration"}
        message={locale === "zh" ? `确定删除配置「${confirmAction?.profile.name}」？此操作不可撤销。` : `Delete "${confirmAction?.profile.name}"? This cannot be undone.`}
        confirmText={locale === "zh" ? "删除" : "Delete"}
        variant="destructive"
        onConfirm={() => {
          if (confirmAction) void doDelete(confirmAction.profile);
          setConfirmAction(null);
        }}
        onCancel={() => setConfirmAction(null)}
      />
    </div>
  );
}
