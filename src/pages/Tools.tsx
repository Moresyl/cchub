import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Terminal, Code, Sparkles, Globe, Cat, Monitor } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import type { DetectedTool } from "../types/skills";

type ToolTab = "claude" | "codex";

const PERMISSION_LEVELS = [
  {
    level: 0,
    label_zh: "严格",
    label_en: "Strict",
    desc_zh: "每次操作都需要确认，最安全",
    desc_en: "Every action requires confirmation",
    color: "#ef4444",
  },
  {
    level: 1,
    label_zh: "标准",
    label_en: "Standard",
    desc_zh: "允许读取文件和搜索，写操作需确认",
    desc_en: "Allow read & search, write requires confirmation",
    color: "#eab308",
  },
  {
    level: 2,
    label_zh: "宽松",
    label_en: "Relaxed",
    desc_zh: "允许读写文件和搜索，仅命令执行需确认",
    desc_en: "Allow read/write & search, only Bash needs confirmation",
    color: "#3b82f6",
  },
  {
    level: 3,
    label_zh: "全部允许",
    label_en: "Allow All",
    desc_zh: "跳过所有权限确认，包括命令执行和 MCP",
    desc_en: "Skip all permission prompts, including Bash and MCP",
    color: "#22c55e",
  },
];

const TOOL_ICONS: Record<string, typeof Monitor> = {
  claude: Terminal,
  codex: Code,
  gemini: Sparkles,
  opencode: Globe,
  openclaw: Cat,
};

export default function Tools() {
  const [tab, setTab] = useState<ToolTab>("claude");
  const [permLevel, setPermLevel] = useState(0);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [tools, setTools] = useState<DetectedTool[]>([]);
  const locale = getLocale();

  useEffect(() => {
    loadData();
  }, []);

  async function loadData() {
    setLoading(true);
    try {
      const [level, detectedTools] = await Promise.all([
        invoke<number>("get_claude_permissions_level"),
        invoke<DetectedTool[]>("detect_tools"),
      ]);
      setPermLevel(level);
      setTools(detectedTools);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }

  async function handleSetPermLevel(level: number) {
    setSaving(true);
    try {
      await invoke("set_claude_permissions_level", { level });
      setPermLevel(level);
      showToast("success", locale === "zh" ? "权限已更新" : "Permissions updated");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `设置失败: ${e}` : `Failed: ${e}`);
    }
    finally { setSaving(false); }
  }

  const installedTools = tools.filter(t => t.installed);
  const currentPerm = PERMISSION_LEVELS[permLevel] || PERMISSION_LEVELS[0];

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
          <h2 className="page-title">{locale === "zh" ? "工具" : "Tools"}</h2>
          <p className="page-subtitle">
            {locale === "zh" ? "管理 AI 编程工具的配置和权限" : "Manage AI coding tool settings and permissions"}
          </p>
        </div>
      </div>

      {/* Tool Tabs */}
      <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
        {(["claude", "codex"] as ToolTab[]).map(toolId => {
          const Icon = TOOL_ICONS[toolId] || Monitor;
          const tool = tools.find(t => t.id === toolId);
          return (
            <button
              key={toolId}
              className={`btn btn-sm ${tab === toolId ? "btn-primary" : "btn-secondary"}`}
              onClick={() => setTab(toolId)}
              style={{ gap: 6, textTransform: "capitalize" }}
            >
              <Icon size={14} />
              {tool?.name || toolId}
            </button>
          );
        })}
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {tab === "claude" && (
          <div>
            {/* Permissions Slider */}
            <div className="section-card" style={{ padding: 24 }}>
              <h3 style={{ fontSize: 15, fontWeight: 700, marginBottom: 4 }}>
                {locale === "zh" ? "权限模式" : "Permission Mode"}
              </h3>
              <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 20 }}>
                {locale === "zh"
                  ? "控制 Claude Code 执行操作时的权限确认级别"
                  : "Control the permission confirmation level for Claude Code actions"}
              </p>

              {/* Current Level Display */}
              <div style={{
                display: "flex", alignItems: "center", gap: 12,
                padding: "14px 18px", borderRadius: 8,
                background: "var(--bg-elevated)", marginBottom: 24,
                border: `1px solid ${currentPerm.color}30`,
              }}>
                <div style={{
                  width: 10, height: 10, borderRadius: "50%",
                  background: currentPerm.color,
                  boxShadow: `0 0 8px ${currentPerm.color}60`,
                  flexShrink: 0,
                }} />
                <div>
                  <span style={{ fontSize: 14, fontWeight: 600 }}>
                    {locale === "zh" ? currentPerm.label_zh : currentPerm.label_en}
                  </span>
                  <span style={{ fontSize: 12, color: "var(--text-muted)", marginLeft: 10 }}>
                    {locale === "zh" ? currentPerm.desc_zh : currentPerm.desc_en}
                  </span>
                </div>
              </div>

              {/* Slider */}
              <div style={{ position: "relative", padding: "0 8px" }}>
                {/* Track */}
                <div style={{
                  height: 6, borderRadius: 3,
                  background: "var(--bg-badge)",
                  position: "relative",
                }}>
                  <div style={{
                    position: "absolute", left: 0, top: 0, height: "100%",
                    width: `${(permLevel / 3) * 100}%`,
                    borderRadius: 3,
                    background: `linear-gradient(90deg, ${PERMISSION_LEVELS[0].color}, ${currentPerm.color})`,
                    transition: "width 0.2s ease",
                  }} />
                </div>

                {/* Slider Input */}
                <input
                  type="range"
                  min={0}
                  max={3}
                  step={1}
                  value={permLevel}
                  disabled={saving}
                  onChange={e => handleSetPermLevel(Number(e.target.value))}
                  style={{
                    position: "absolute", top: -8, left: 0, width: "100%",
                    height: 24, opacity: 0, cursor: "pointer",
                  }}
                />

                {/* Thumb */}
                <div style={{
                  position: "absolute",
                  top: -5,
                  left: `calc(${(permLevel / 3) * 100}% - 8px)`,
                  width: 16, height: 16, borderRadius: "50%",
                  background: currentPerm.color,
                  border: "2px solid var(--bg-app)",
                  boxShadow: `0 0 6px ${currentPerm.color}60`,
                  transition: "left 0.2s ease",
                  pointerEvents: "none",
                }} />
              </div>

              {/* Level Labels */}
              <div style={{ display: "flex", justifyContent: "space-between", marginTop: 14 }}>
                {PERMISSION_LEVELS.map(pl => (
                  <button
                    key={pl.level}
                    onClick={() => handleSetPermLevel(pl.level)}
                    disabled={saving}
                    style={{
                      background: "none", border: "none", cursor: "pointer",
                      fontSize: 11, fontWeight: permLevel === pl.level ? 700 : 400,
                      color: permLevel === pl.level ? pl.color : "var(--text-muted)",
                      transition: "all 0.15s ease",
                      padding: "4px 0",
                    }}
                  >
                    {locale === "zh" ? pl.label_zh : pl.label_en}
                  </button>
                ))}
              </div>
            </div>

            {/* Permission Details */}
            <div className="section-card" style={{ padding: 24, marginTop: 16 }}>
              <h3 style={{ fontSize: 15, fontWeight: 700, marginBottom: 16 }}>
                {locale === "zh" ? "各级别权限详情" : "Permission Level Details"}
              </h3>
              <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                {PERMISSION_LEVELS.map(pl => (
                  <div
                    key={pl.level}
                    onClick={() => handleSetPermLevel(pl.level)}
                    style={{
                      display: "flex", alignItems: "center", gap: 14,
                      padding: "12px 16px", borderRadius: 8,
                      background: permLevel === pl.level ? "var(--bg-elevated)" : "transparent",
                      border: permLevel === pl.level ? `1px solid ${pl.color}30` : "1px solid transparent",
                      cursor: "pointer",
                      transition: "all 0.15s ease",
                    }}
                  >
                    <div style={{
                      width: 8, height: 8, borderRadius: "50%",
                      background: pl.color,
                      opacity: permLevel >= pl.level ? 1 : 0.3,
                      flexShrink: 0,
                    }} />
                    <div style={{ flex: 1 }}>
                      <span style={{ fontSize: 13, fontWeight: 600, color: permLevel === pl.level ? "var(--text-primary)" : "var(--text-secondary)" }}>
                        {locale === "zh" ? pl.label_zh : pl.label_en}
                      </span>
                      <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 2 }}>
                        {locale === "zh" ? pl.desc_zh : pl.desc_en}
                      </p>
                    </div>
                    {permLevel === pl.level && (
                      <span className="badge badge-success" style={{ fontSize: 10 }}>
                        {locale === "zh" ? "当前" : "Active"}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {tab === "codex" && (
          <div className="section-card" style={{ padding: 24 }}>
            <h3 style={{ fontSize: 15, fontWeight: 700, marginBottom: 8 }}>
              Codex CLI
            </h3>
            <p style={{ fontSize: 13, color: "var(--text-muted)" }}>
              {locale === "zh" ? "Codex CLI 工具设置即将推出" : "Codex CLI tool settings coming soon"}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
