import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw, Save, Trash2, Check, Eye, X, ArrowRightLeft,
  Search, Terminal, Code, Monitor, Sparkles, Globe, Wind,
} from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import CodeEditor from "../components/CodeEditor";

interface ConfigProfile {
  id: string;
  name: string;
  tool_id: string;
  config_snapshot: string;
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
  codex: Monitor,
  gemini: Sparkles,
  cursor: Code,
  windsurf: Wind,
  opencode: Globe,
};

function formatTime(value: string | null) {
  if (!value) return "";
  return value.replace("T", " ").slice(0, 19);
}

function getEditorLanguage(profile: ConfigProfile): "json" | "toml" {
  if (profile.tool_id === "codex") {
    try {
      JSON.parse(profile.config_snapshot);
      return "json";
    } catch {
      return "toml";
    }
  }
  return "json";
}

function getConfigHint(toolId: string, locale: string) {
  if (toolId === "codex") {
    return locale === "zh" ? "包含 auth.json 和 config.toml" : "Includes auth.json and config.toml";
  }
  if (toolId === "gemini") {
    return locale === "zh" ? "包含 .env 和 settings.json" : "Includes .env and settings.json";
  }
  return locale === "zh" ? "完整配置" : "Full configuration";
}

export default function Profiles() {
  const [profiles, setProfiles] = useState<ConfigProfile[]>([]);
  const [tools, setTools] = useState<DetectedTool[]>([]);
  const [activeIds, setActiveIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState("");
  const [newTool, setNewTool] = useState("claude");
  const [saving, setSaving] = useState(false);
  const [applying, setApplying] = useState<string | null>(null);
  const [preview, setPreview] = useState<ConfigProfile | null>(null);
  const [filterTool, setFilterTool] = useState("all");
  const [search, setSearch] = useState("");
  const [activeOnly, setActiveOnly] = useState(false);
  const locale = getLocale();

  useEffect(() => { void load(); }, []);

  async function load() {
    setLoading(true);
    try {
      const [nextProfiles, nextTools, nextActiveIds] = await Promise.all([
        invoke<ConfigProfile[]>("get_config_profiles"),
        invoke<DetectedTool[]>("detect_tools"),
        invoke<string[]>("get_active_config_profile_ids"),
      ]);
      setProfiles(nextProfiles);
      setTools(nextTools);
      setActiveIds(nextActiveIds);
      setPreview((prev) => prev ? nextProfiles.find((item) => item.id === prev.id) || null : null);
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

  async function handleSave() {
    if (!newName.trim() || saving) return;
    setSaving(true);
    try {
      const configContent = await invoke<string>("read_tool_config", { toolId: newTool });
      await invoke("save_config_profile", {
        name: newName.trim(),
        toolId: newTool,
        configSnapshot: configContent,
      });
      setNewName("");
      await load();
      showToast("success", locale === "zh" ? "配置已保存" : "Configuration saved");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `保存失败: ${e}` : `Save failed: ${e}`);
    } finally {
      setSaving(false);
    }
  }

  async function handleApply(profile: ConfigProfile) {
    const msg = locale === "zh"
      ? `确定切换到配置「${profile.name}」？\n\n这会覆盖 ${profile.tool_id} 的当前配置文件。`
      : `Switch to "${profile.name}"?\n\nThis will overwrite the current ${profile.tool_id} configuration.`;
    if (!window.confirm(msg)) return;
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
    const msg = locale === "zh" ? `确定删除配置「${profile.name}」？` : `Delete "${profile.name}"?`;
    if (!window.confirm(msg)) return;
    try {
      await invoke("delete_config_profile", { id: profile.id });
      setPreview((prev) => prev?.id === profile.id ? null : prev);
      await load();
      showToast("success", locale === "zh" ? "配置已删除" : "Configuration deleted");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `删除失败: ${e}` : `Delete failed: ${e}`);
    }
  }

  const activeIdSet = useMemo(() => new Set(activeIds), [activeIds]);
  const installedTools = useMemo(() => tools.filter((tool) => tool.installed), [tools]);

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
        if (filterTool !== "all" && profile.tool_id !== filterTool) return false;
        if (activeOnly && !activeIdSet.has(profile.id)) return false;
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
  }, [profiles, filterTool, activeOnly, search, activeIdSet]);

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

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{locale === "zh" ? "配置切换" : "Config Profiles"}</h2>
          <p className="page-subtitle">
            {locale === "zh"
              ? `共 ${profiles.length} 个配置，当前命中 ${activeIds.length} 个`
              : `${profiles.length} configurations, ${activeIds.length} currently matched`}
          </p>
        </div>
        <button className="btn btn-secondary btn-sm" onClick={() => void load()} style={{ gap: 6 }}>
          <RefreshCw size={14} />
          {locale === "zh" ? "刷新" : "Refresh"}
        </button>
      </div>

      <div className="section-card" style={{ marginBottom: 18, padding: 18 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 14, flexWrap: "wrap" }}>
          <div>
            <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
              {locale === "zh" ? "新增配置" : "New Configuration"}
            </div>
            <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
              {locale === "zh" ? "保存当前工具的完整配置，后续一键切换。" : "Save the complete current configuration for quick switching."}
            </div>
          </div>
        </div>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 12 }}>
          {installedTools.map((tool) => {
            const Icon = TOOL_ICONS[tool.id] || Monitor;
            const selected = newTool === tool.id;
            return (
              <button
                key={tool.id}
                className={`btn btn-sm ${selected ? "btn-primary" : "btn-secondary"}`}
                onClick={() => setNewTool(tool.id)}
                style={{ gap: 6 }}
              >
                <Icon size={14} />
                {tool.name}
              </button>
            );
          })}
        </div>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <input
            className="input"
            style={{ flex: 1, minWidth: 240, fontSize: 13 }}
            placeholder={locale === "zh" ? "配置名称，例如：生产环境、备用配置" : "Configuration name, e.g. Production, Backup"}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleSave();
            }}
          />
          <button
            className="btn btn-primary btn-sm"
            disabled={!newName.trim() || saving || installedTools.length === 0}
            onClick={() => void handleSave()}
            style={{ gap: 6 }}
          >
            {saving ? <div className="spinner" style={{ width: 14, height: 14 }} /> : <Save size={14} />}
            {locale === "zh" ? "保存配置" : "Save"}
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
            placeholder={locale === "zh" ? "搜索配置名称、工具或内容..." : "Search name, tool, or content..."}
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
          <button
            className={`tab-item ${filterTool === "all" ? "active" : ""}`}
            onClick={() => setFilterTool("all")}
          >
            {locale === "zh" ? "全部" : "All"} ({profiles.length})
          </button>
          {tools.map((tool) => (
            <button
              key={tool.id}
              className={`tab-item ${filterTool === tool.id ? "active" : ""}`}
              onClick={() => setFilterTool(tool.id)}
              style={{ opacity: tool.installed || (toolCounts[tool.id] || 0) > 0 ? 1 : 0.55 }}
            >
              {tool.name} ({toolCounts[tool.id] || 0})
            </button>
          ))}
          <button
            className={`tab-item ${activeOnly ? "active" : ""}`}
            onClick={() => setActiveOnly((prev) => !prev)}
          >
            {locale === "zh" ? "当前生效" : "Current"} ({activeIds.length})
          </button>
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, display: "flex", gap: 20 }}>
        <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 10 }}>
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
                  ? "先保存一份当前配置，之后就可以在这里直接切换和删除。"
                  : "Save a configuration first, then switch or delete it here."}
              </p>
            </div>
          ) : (
            filteredProfiles.map((profile) => {
              const Icon = TOOL_ICONS[profile.tool_id] || Monitor;
              const isActive = activeIdSet.has(profile.id);
              const isSelected = preview?.id === profile.id;
              return (
                <div
                  key={profile.id}
                  className={`card card-interactive ${isSelected ? "selected" : ""}`}
                  style={{
                    padding: "16px 18px",
                    borderColor: isActive ? "var(--success)" : undefined,
                    boxShadow: isActive ? "0 0 0 1px color-mix(in srgb, var(--success) 30%, transparent)" : undefined,
                  }}
                  onClick={() => setPreview(profile)}
                >
                  <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 12 }}>
                    <div style={{ display: "flex", gap: 12, minWidth: 0, flex: 1 }}>
                      <div className="icon-box" style={{ width: 36, height: 36, borderRadius: 8 }}>
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
                              {locale === "zh" ? "当前生效" : "Current"}
                            </span>
                          )}
                        </div>
                        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
                          {getConfigHint(profile.tool_id, locale)}
                        </div>
                        <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>
                          {formatTime(profile.updated_at || profile.created_at)}
                        </div>
                      </div>
                    </div>

                    <div
                      style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}
                      onClick={(e) => e.stopPropagation()}
                    >
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
                        onClick={() => setPreview(profile)}
                        title={locale === "zh" ? "预览" : "Preview"}
                      >
                        <Eye size={14} />
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

        {preview && (
          <div style={{ width: 520, overflowY: "auto", flexShrink: 0 }}>
            <div className="section-card" style={{ position: "sticky", top: 0 }}>
              <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 12, marginBottom: 16 }}>
                <div>
                  <h3 style={{ fontSize: 15, fontWeight: 700 }}>{preview.name}</h3>
                  <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginTop: 6 }}>
                    <span className="badge badge-muted" style={{ textTransform: "capitalize" }}>{preview.tool_id}</span>
                    {activeIdSet.has(preview.id) && (
                      <span className="badge badge-success">{locale === "zh" ? "当前生效" : "Current"}</span>
                    )}
                    <span className="badge badge-muted">{(preview.config_snapshot.length / 1024).toFixed(1)} KB</span>
                  </div>
                  <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 10 }}>
                    {getConfigHint(preview.tool_id, locale)}
                  </div>
                </div>

                <div style={{ display: "flex", gap: 6 }}>
                  <button className="btn btn-primary btn-sm" onClick={() => void handleApply(preview)} style={{ gap: 5 }}>
                    <ArrowRightLeft size={13} />
                    {locale === "zh" ? "切换到此配置" : "Apply"}
                  </button>
                  <button className="btn btn-ghost btn-icon-sm" onClick={() => setPreview(null)}>
                    <X size={16} />
                  </button>
                </div>
              </div>

              <CodeEditor
                value={preview.config_snapshot}
                language={getEditorLanguage(preview)}
                readOnly
                minHeight={340}
              />

              <div style={{ display: "flex", justifyContent: "space-between", marginTop: 12, gap: 8 }}>
                <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                  {formatTime(preview.updated_at || preview.created_at)}
                </div>
                <button
                  className="btn btn-danger-ghost btn-sm"
                  onClick={() => void handleDelete(preview)}
                  style={{ gap: 5 }}
                >
                  <Trash2 size={13} />
                  {locale === "zh" ? "删除此配置" : "Delete"}
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
