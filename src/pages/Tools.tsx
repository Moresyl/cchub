import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Terminal, Code, Download, RefreshCw, Cat, Globe, Search } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import OmoConfigSection from "../components/OmoConfigSection";
import type { DetectedTool } from "../types/skills";
import { fetchVisibleApps, type ManagedAppId } from "../lib/appPreferences";
import {
  buildStructuredConfig,
  createDefaultStructuredFields,
  parseStructuredConfig,
  type OpenClawApiProtocol,
  type StructuredDraftFields,
} from "../lib/configProfiles";

type ToolTab = "claude" | "codex" | "opencode" | "openclaw";

interface HudStatus {
  installed: boolean;
  version: string;
  indexJsPath: string;
  statuslineEnabled: boolean;
  hudConfig: HudConfig;
}

interface HudConfig {
  layout?: string;
  pathLevels?: number;
  gitStatus?: {
    enabled?: boolean;
    showDirty?: boolean;
    showAheadBehind?: boolean;
    showFileStats?: boolean;
  };
  display?: {
    showModel?: boolean;
    showContextBar?: boolean;
    showConfigCounts?: boolean;
    showDuration?: boolean;
    showUsage?: boolean;
    usageBarEnabled?: boolean;
    showTokenBreakdown?: boolean;
    showTools?: boolean;
    showAgents?: boolean;
    showTodos?: boolean;
  };
}

interface OpenClawDailyMemoryEntry {
  path: string;
  file_name: string;
  source: string;
  project_name: string | null;
  modified_at: string | null;
  preview: string;
}

const DEFAULT_HUD_CONFIG: HudConfig = {
  layout: "separators",
  pathLevels: 2,
  gitStatus: { enabled: true, showDirty: true, showAheadBehind: false, showFileStats: false },
  display: { showModel: true, showContextBar: true, showConfigCounts: true, showDuration: true, showUsage: true, usageBarEnabled: true, showTokenBreakdown: true, showTools: true, showAgents: true, showTodos: true },
};

const PERM_LEVELS = [
  { label_zh: "严格", label_en: "Strict", label_ja: "厳格", color: "#ef4444" },
  { label_zh: "标准", label_en: "Standard", label_ja: "標準", color: "#eab308" },
  { label_zh: "宽松", label_en: "Relaxed", label_ja: "緩和", color: "#3b82f6" },
  { label_zh: "全部允许", label_en: "Allow All", label_ja: "すべて許可", color: "#22c55e" },
];

const PERM_DESC_ZH = ["每次操作都确认", "允许读取，写操作确认", "允许读写，仅 Bash 确认", "跳过所有确认"];
const PERM_DESC_EN = ["Confirm every action", "Allow read, confirm write", "Allow read/write, confirm Bash", "Skip all prompts"];
const PERM_DESC_JA = ["毎回すべて確認", "読み取りを許可し、書き込みは確認", "読み書きを許可し、Bash のみ確認", "すべての確認をスキップ"];
const OPENCLAW_PROTOCOL_OPTIONS: OpenClawApiProtocol[] = [
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
  "bedrock-converse-stream",
];

export default function Tools() {
  const [tab, setTab] = useState<ToolTab>("claude");
  const [permLevel, setPermLevel] = useState(0);
  const [autoUpdate, setAutoUpdate] = useState("latest");
  const [claudeModel, setClaudeModel] = useState("");
  const [toolSearch, setToolSearch] = useState(false);
  const [codexApproval, setCodexApproval] = useState("suggest");
  const [codexReasoning, setCodexReasoning] = useState("medium");
  const [codexDisableStorage, setCodexDisableStorage] = useState(false);
  const [codexContextWindow1M, setCodexContextWindow1M] = useState(false);
  const [openClawDraft, setOpenClawDraft] = useState<StructuredDraftFields>(() => createDefaultStructuredFields("openclaw"));
  const [openClawLoading, setOpenClawLoading] = useState(false);
  const [openClawSaving, setOpenClawSaving] = useState(false);
  const [openClawMemoryQuery, setOpenClawMemoryQuery] = useState("");
  const [openClawMemoryEntries, setOpenClawMemoryEntries] = useState<OpenClawDailyMemoryEntry[]>([]);
  const [openClawMemoryLoading, setOpenClawMemoryLoading] = useState(false);
  const [openClawMemorySelectedPath, setOpenClawMemorySelectedPath] = useState<string | null>(null);
  const [openClawMemoryContent, setOpenClawMemoryContent] = useState("");
  const [openClawMemoryLoadingContent, setOpenClawMemoryLoadingContent] = useState(false);
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>(["claude", "codex", "gemini", "opencode", "openclaw"]);
  const [loading, setLoading] = useState(true);
  const [tools, setTools] = useState<DetectedTool[]>([]);
  const [hudStatus, setHudStatus] = useState<HudStatus | null>(null);
  const [hudInstalling, setHudInstalling] = useState(false);
  const [hudUpdateInfo, setHudUpdateInfo] = useState<{ currentVersion: string; latestVersion: string; hasUpdate: boolean } | null>(null);
  const [hudUpdating, setHudUpdating] = useState(false);
  const [hudChecking, setHudChecking] = useState(false);
  const locale = getLocale();
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  );

  const visibleTabs = (["claude", "codex", "opencode", "openclaw"] as ToolTab[]).filter((id) => visibleApps.includes(id));

  useEffect(() => { loadData(); }, []);
  useEffect(() => {
    if (visibleTabs.length === 0) return;
    if (!visibleTabs.includes(tab)) setTab(visibleTabs[0]);
  }, [tab, visibleTabs]);
  useEffect(() => {
    if (tab === "openclaw" && tools.find((tool) => tool.id === "openclaw")?.installed) {
      void loadOpenClawConfig();
      void loadOpenClawDailyMemory();
    }
  }, [tab, tools]);

  async function loadData() {
    setLoading(true);
    try {
      const [level, channel, model, toolSearchEnabled, codexSettings, detectedTools, hud, nextVisibleApps] = await Promise.all([
        invoke<number>("get_claude_permissions_level").catch(() => 0),
        invoke<string>("get_claude_auto_update").catch(() => "latest"),
        invoke<string>("get_claude_model").catch(() => ""),
        invoke<boolean>("get_claude_tool_search").catch(() => false),
        invoke<{ approval_mode: string; reasoning_effort: string; disable_response_storage: boolean; context_window_1m: boolean }>("get_codex_settings").catch(() => ({
          approval_mode: "suggest", reasoning_effort: "medium", disable_response_storage: false, context_window_1m: false,
        })),
        invoke<DetectedTool[]>("detect_tools").catch(() => []),
        invoke<HudStatus>("get_claude_hud_status").catch(() => null),
        fetchVisibleApps(),
      ]);
      setPermLevel(level);
      setAutoUpdate(channel);
      setClaudeModel(model);
      setToolSearch(toolSearchEnabled);
      setCodexApproval(codexSettings.approval_mode);
      setCodexReasoning(codexSettings.reasoning_effort);
      setCodexDisableStorage(codexSettings.disable_response_storage);
      setCodexContextWindow1M(codexSettings.context_window_1m);
      setTools(detectedTools);
      setHudStatus(hud);
      setVisibleApps(nextVisibleApps);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }

  async function setClaudeSetting(fn: string, args: Record<string, unknown>, onSuccess: () => void) {
    try {
      await invoke(fn, args);
      onSuccess();
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) { showToast("error", `${e}`); }
  }

  async function setCodex(key: string, value: string) {
    try {
      await invoke("set_codex_setting", { key, value });
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) { showToast("error", `${e}`); }
  }

  async function handleInstallHud() {
    setHudInstalling(true);
    try {
      await invoke("install_claude_hud");
      const hud = await invoke<HudStatus>("get_claude_hud_status");
      setHudStatus(hud);
      showToast("success", uiText("claude-hud 安装成功", "claude-hud installed", "claude-hud をインストールしました"));
    } catch (e) {
      showToast("error", uiText(`安装失败: ${e}`, `Install failed: ${e}`, `インストールに失敗しました: ${e}`));
    } finally {
      setHudInstalling(false);
    }
  }

  async function checkHudUpdate() {
    setHudChecking(true);
    try {
      const info = await invoke<{ currentVersion: string; latestVersion: string; hasUpdate: boolean }>("check_claude_hud_update");
      setHudUpdateInfo(info);
      if (!info.hasUpdate) {
        showToast("success", uiText("已是最新版本", "Already up to date", "すでに最新です"));
      }
    } catch (e) {
      showToast("error", uiText(`检查更新失败: ${e}`, `Check failed: ${e}`, `更新確認に失敗しました: ${e}`));
    } finally {
      setHudChecking(false);
    }
  }

  async function handleUpdateHud() {
    setHudUpdating(true);
    try {
      const result = await invoke<{ version: string; skipped: boolean }>("update_claude_hud");
      const hud = await invoke<HudStatus>("get_claude_hud_status");
      setHudStatus(hud);
      setHudUpdateInfo(null);
      if (result.skipped) {
        showToast("success", uiText("已是最新版本", "Already up to date", "すでに最新です"));
      } else {
        showToast("success", uiText(`已更新到 v${result.version}`, `Updated to v${result.version}`, `v${result.version} に更新しました`));
      }
    } catch (e) {
      showToast("error", uiText(`更新失败: ${e}`, `Update failed: ${e}`, `更新に失敗しました: ${e}`));
    } finally {
      setHudUpdating(false);
    }
  }

  async function toggleStatusLine(enabled: boolean) {
    try {
      await invoke("set_claude_statusline", { enabled });
      const hud = await invoke<HudStatus>("get_claude_hud_status");
      setHudStatus(hud);
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) { showToast("error", `${e}`); }
  }

  async function updateHudConfig(patch: Partial<HudConfig>) {
    if (!hudStatus) return;
    const current = hudStatus.hudConfig || DEFAULT_HUD_CONFIG;
    const updated: HudConfig = {
      ...current,
      ...patch,
      gitStatus: { ...current.gitStatus, ...patch.gitStatus },
      display: { ...current.display, ...patch.display },
    };
    try {
      await invoke("set_claude_hud_config", { config: updated });
      setHudStatus({ ...hudStatus, hudConfig: updated });
      showToast("success", uiText("已更新", "Updated", "更新しました"));
    } catch (e) { showToast("error", `${e}`); }
  }

  function updateOpenClawDraft(next: Partial<StructuredDraftFields>) {
    setOpenClawDraft((current) => ({ ...current, ...next }));
  }

  async function loadOpenClawConfig() {
    setOpenClawLoading(true);
    try {
      const content = await invoke<string>("read_tool_config", { toolId: "openclaw" });
      setOpenClawDraft(parseStructuredConfig("openclaw", content));
    } catch (e) {
      console.error(e);
      setOpenClawDraft(createDefaultStructuredFields("openclaw"));
      showToast("error", uiText(`读取 OpenClaw 配置失败: ${e}`, `Failed to load OpenClaw config: ${e}`, `OpenClaw 設定の読み込みに失敗しました: ${e}`));
    } finally {
      setOpenClawLoading(false);
    }
  }

  async function saveOpenClawConfig() {
    setOpenClawSaving(true);
    try {
      await invoke("write_tool_config", {
        toolId: "openclaw",
        content: buildStructuredConfig("openclaw", openClawDraft),
      });
      showToast("success", uiText("OpenClaw 配置已保存", "OpenClaw config saved", "OpenClaw 設定を保存しました"));
    } catch (e) {
      showToast("error", uiText(`保存 OpenClaw 配置失败: ${e}`, `Failed to save OpenClaw config: ${e}`, `OpenClaw 設定の保存に失敗しました: ${e}`));
    } finally {
      setOpenClawSaving(false);
    }
  }

  async function openOpenClawDailyMemoryEntry(entry: OpenClawDailyMemoryEntry) {
    setOpenClawMemorySelectedPath(entry.path);
    setOpenClawMemoryLoadingContent(true);
    try {
      const content = await invoke<string>("read_openclaw_daily_memory_content", { path: entry.path });
      setOpenClawMemoryContent(content);
    } catch (e) {
      setOpenClawMemoryContent("");
      showToast(
        "error",
        uiText(
          `读取 Daily Memory 失败: ${e}`,
          `Failed to load Daily Memory entry: ${e}`,
          `Daily Memory の読み込みに失敗しました: ${e}`,
        ),
      );
    } finally {
      setOpenClawMemoryLoadingContent(false);
    }
  }

  async function loadOpenClawDailyMemory(query = openClawMemoryQuery) {
    setOpenClawMemoryLoading(true);
    try {
      const entries = await invoke<OpenClawDailyMemoryEntry[]>("search_openclaw_daily_memory", {
        query,
        limit: 40,
      });
      setOpenClawMemoryEntries(entries);
      const nextSelectedPath = entries.some((entry) => entry.path === openClawMemorySelectedPath)
        ? openClawMemorySelectedPath
        : (entries[0]?.path ?? null);
      const nextEntry = nextSelectedPath
        ? entries.find((entry) => entry.path === nextSelectedPath) ?? null
        : null;
      setOpenClawMemorySelectedPath(nextSelectedPath);
      if (nextEntry) {
        await openOpenClawDailyMemoryEntry(nextEntry);
      } else {
        setOpenClawMemoryContent("");
      }
    } catch (e) {
      setOpenClawMemoryEntries([]);
      setOpenClawMemorySelectedPath(null);
      setOpenClawMemoryContent("");
      showToast(
        "error",
        uiText(
          `搜索 Daily Memory 失败: ${e}`,
          `Failed to search Daily Memory: ${e}`,
          `Daily Memory の検索に失敗しました: ${e}`,
        ),
      );
    } finally {
      setOpenClawMemoryLoading(false);
    }
  }

  if (loading) {
    return <div className="loading-center"><div className="spinner" /><span style={{ fontSize: 13, color: "var(--text-muted)" }}>{uiText("加载中...", "Loading...", "読み込み中...")}</span></div>;
  }

  const perm = PERM_LEVELS[permLevel] || PERM_LEVELS[0];
  const hc = hudStatus?.hudConfig || DEFAULT_HUD_CONFIG;

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{uiText("工具", "Tools", "ツール")}</h2>
          <p className="page-subtitle">{uiText("管理 AI 编程工具的配置和权限", "Manage AI coding tool settings", "AI コーディングツールの設定と権限を管理")}</p>
        </div>
      </div>

        {/* Tabs */}
      <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
        {visibleTabs.map(id => {
          const Icon = id === "claude" ? Terminal : id === "codex" ? Code : id === "opencode" ? Globe : Cat;
          const tool = tools.find(t => t.id === id);
          const installed = tool?.installed ?? false;
          return (
            <button key={id} className={`btn btn-sm ${tab === id ? "btn-primary" : "btn-secondary"}`}
              onClick={() => setTab(id)} style={{ gap: 6, opacity: installed ? 1 : 0.5 }}>
              <Icon size={14} />{tool?.name || id}
              {!installed && <span style={{ fontSize: 9, color: "var(--text-muted)" }}>({uiText("未安装", "N/A", "未インストール")})</span>}
            </button>
          );
        })}
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
        {visibleTabs.length === 0 && (
          <div className="card" style={{ padding: "40px 20px", textAlign: "center", marginBottom: 12 }}>
            <p style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
              {uiText("当前已隐藏所有工具页签", "All tool tabs are currently hidden", "すべてのツールタブは現在非表示です")}
            </p>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {uiText("可在设置页的 App 可见性中重新开启", "Re-enable them from Settings > App Visibility", "Settings > App Visibility から再表示できます")}
            </p>
          </div>
        )}

        {/* Not installed hint */}
        {visibleTabs.length > 0 && !(tools.find(t => t.id === tab)?.installed) && (
          <div className="card" style={{ padding: "40px 20px", textAlign: "center" }}>
            <p style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
              {uiText(
                `${tab === "claude" ? "Claude Code" : tab === "codex" ? "Codex CLI" : tab === "opencode" ? "OpenCode" : "OpenClaw"} 未安装`,
                `${tab === "claude" ? "Claude Code" : tab === "codex" ? "Codex CLI" : tab === "opencode" ? "OpenCode" : "OpenClaw"} not installed`,
                `${tab === "claude" ? "Claude Code" : tab === "codex" ? "Codex CLI" : tab === "opencode" ? "OpenCode" : "OpenClaw"} は未インストールです`,
              )}
            </p>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {uiText("安装后即可在此管理工具设置", "Install it to manage settings here", "インストール後にここで設定を管理できます")}
            </p>
          </div>
        )}

        {tab === "claude" && tools.find(t => t.id === "claude")?.installed && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {/* Permission Slider */}
            <div className="card" style={{ padding: "16px 18px", display: "flex", alignItems: "center", gap: 20 }}>
              <div style={{ flex: 1 }}>
                <h4 style={{ fontSize: 13, fontWeight: 700, marginBottom: 10 }}>{uiText("权限模式", "Permission Mode", "権限モード")}</h4>
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
                  <span style={{ width: 7, height: 7, borderRadius: "50%", background: perm.color, boxShadow: `0 0 5px ${perm.color}50` }} />
                  <span style={{ fontSize: 12, fontWeight: 600 }}>{uiText(perm.label_zh, perm.label_en, perm.label_ja)}</span>
                  <span style={{ fontSize: 11, color: "var(--text-muted)" }}>— {uiText(PERM_DESC_ZH[permLevel], PERM_DESC_EN[permLevel], PERM_DESC_JA[permLevel])}</span>
                </div>
                <div style={{ position: "relative", height: 5, borderRadius: 3, background: "var(--bg-badge)" }}>
                  <div style={{ position: "absolute", left: 0, top: 0, height: "100%", width: `${(permLevel / 3) * 100}%`, borderRadius: 3, background: `linear-gradient(90deg, #ef4444, ${perm.color})`, transition: "width 0.2s" }} />
                  <input type="range" min={0} max={3} step={1} value={permLevel}
                    onChange={e => { const v = Number(e.target.value); setPermLevel(v); setClaudeSetting("set_claude_permissions_level", { level: v }, () => {}); }}
                    style={{ position: "absolute", top: -8, left: 0, width: "100%", height: 22, opacity: 0, cursor: "pointer" }} />
                  <div style={{ position: "absolute", top: -5, left: `calc(${(permLevel / 3) * 100}% - 7px)`, width: 14, height: 14, borderRadius: "50%", background: perm.color, border: "2px solid var(--bg-app)", boxShadow: `0 0 5px ${perm.color}60`, transition: "left 0.2s", pointerEvents: "none" }} />
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", marginTop: 8 }}>
                  {PERM_LEVELS.map((pl, i) => (
                    <span key={i} style={{ fontSize: 10, color: permLevel === i ? pl.color : "var(--text-muted)", fontWeight: permLevel === i ? 700 : 400, cursor: "pointer" }}
                      onClick={() => { setPermLevel(i); setClaudeSetting("set_claude_permissions_level", { level: i }, () => {}); }}>
                      {uiText(pl.label_zh, pl.label_en, pl.label_ja)}
                    </span>
                  ))}
                </div>
              </div>
            </div>

            {/* Bypass Permissions */}
            <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("绕过权限确认", "Bypass Permissions", "権限確認をバイパス")}</h4>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{uiText("跳过所有权限确认，全自动执行", "Skip all permission prompts, fully autonomous", "すべての権限確認をスキップして完全自動で実行します")}</p>
              </div>
              <ToggleSwitch
                value={permLevel === 3}
                onChange={v => {
                  const newLevel = v ? 3 : 0;
                  setPermLevel(newLevel);
                  setClaudeSetting("set_claude_permissions_level", { level: newLevel }, () => {});
                }}
                labelOn="ON"
                labelOff="OFF"
              />
            </div>

            {/* Auto Update */}
            <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("自动更新", "Auto Update", "自動更新")}</h4>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{uiText("Claude Code 更新频道", "Update channel", "Claude Code の更新チャンネル")}</p>
              </div>
              <div style={{ display: "flex", gap: 6 }}>
                {[
                  { value: "latest", label: uiText("最新", "Latest", "最新") },
                  { value: "stable", label: uiText("稳定", "Stable", "安定版") },
                  { value: "disabled", label: uiText("关闭", "Off", "オフ") },
                ].map(opt => (
                  <button key={opt.value}
                    className={`btn btn-xs ${autoUpdate === opt.value ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => { setAutoUpdate(opt.value); setClaudeSetting("set_claude_auto_update", { channel: opt.value }, () => {}); }}>
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Model Selection */}
            <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("模型选择", "Model", "モデル")}</h4>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{uiText("切换默认使用的模型", "Switch default model", "既定モデルを切り替えます")}</p>
              </div>
              <div style={{ display: "flex", gap: 6 }}>
                {[
                  { value: "opus", label: "Opus" },
                  { value: "sonnet", label: "Sonnet" },
                  { value: "haiku", label: "Haiku" },
                ].map(opt => (
                  <button key={opt.value}
                    className={`btn btn-xs ${claudeModel.includes(opt.value) ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => { setClaudeModel(opt.value); setClaudeSetting("set_claude_model", { model: opt.value }, () => {}); }}>
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Tool Search */}
            <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <h4 style={{ fontSize: 13, fontWeight: 700 }}>Tool Search</h4>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{uiText("启用工具搜索功能（实验性）", "Enable tool search (experimental)", "ツール検索機能を有効化します（実験的）")}</p>
              </div>
              <ToggleSwitch
                value={toolSearch}
                onChange={v => { setToolSearch(v); setClaudeSetting("set_claude_tool_search", { enabled: v }, () => {}); }}
                labelOn={uiText("已启用", "Enabled", "有効")}
                labelOff={uiText("已关闭", "Disabled", "無効")}
              />
            </div>

            {/* StatusLine (claude-hud) */}
            <div className="card" style={{ padding: "16px 18px" }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: hudStatus?.installed ? 12 : 0 }}>
                <div>
                  <h4 style={{ fontSize: 13, fontWeight: 700 }}>StatusLine (claude-hud)</h4>
                  <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                    {uiText("终端底部实时状态栏", "Real-time status bar at terminal bottom", "ターミナル下部のリアルタイムステータスバー")}
                    {hudStatus?.installed && hudStatus.version && (
                      <span style={{ marginLeft: 6, fontSize: 10, opacity: 0.7 }}>v{hudStatus.version}</span>
                    )}
                  </p>
                </div>
                {!hudStatus?.installed ? (
                  <button
                    className="btn btn-primary btn-xs"
                    onClick={() => void handleInstallHud()}
                    disabled={hudInstalling}
                    style={{ gap: 5 }}
                  >
                    {hudInstalling ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Download size={12} />}
                    {uiText("安装", "Install", "インストール")}
                  </button>
                ) : (
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    {hudUpdateInfo?.hasUpdate ? (
                      <button
                        className="btn btn-primary btn-xs"
                        onClick={() => void handleUpdateHud()}
                        disabled={hudUpdating}
                        style={{ gap: 5 }}
                      >
                        {hudUpdating ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Download size={12} />}
                        {uiText(`更新到 v${hudUpdateInfo.latestVersion}`, `Update to v${hudUpdateInfo.latestVersion}`, `v${hudUpdateInfo.latestVersion} に更新`)}
                      </button>
                    ) : (
                      <button
                        className="btn btn-secondary btn-xs"
                        onClick={() => void checkHudUpdate()}
                        disabled={hudChecking}
                        title={uiText("检查更新", "Check for updates", "更新を確認")}
                        style={{ gap: 4 }}
                      >
                        {hudChecking ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={12} />}
                        {uiText("检查更新", "Check Update", "更新を確認")}
                      </button>
                    )}
                    <ToggleSwitch
                      value={hudStatus.statuslineEnabled}
                      onChange={v => void toggleStatusLine(v)}
                      labelOn={uiText("已启用", "Enabled", "有効")}
                      labelOff={uiText("已关闭", "Disabled", "無効")}
                    />
                  </div>
                )}
              </div>

              {hudStatus?.installed && (
                <div style={{ borderTop: "1px solid var(--border)", paddingTop: 12 }}>
                  {/* Layout & Path */}
                  <div style={{ display: "flex", gap: 16, marginBottom: 10, alignItems: "center" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      <span style={{ fontSize: 12, color: "var(--text-muted)" }}>{uiText("布局", "Layout", "レイアウト")}</span>
                      <div style={{ display: "flex", gap: 4 }}>
                        {(["default", "separators"] as const).map(layout => (
                          <button key={layout}
                            className={`btn btn-xs ${(hc.layout || "separators") === layout ? "btn-primary" : "btn-secondary"}`}
                            onClick={() => void updateHudConfig({ layout })}
                            style={{ fontSize: 11 }}
                          >
                            {layout === "default"
                              ? uiText("紧凑", "Compact", "コンパクト")
                              : uiText("分隔线", "Separators", "区切り線")}
                          </button>
                        ))}
                      </div>
                    </div>
                    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      <span style={{ fontSize: 12, color: "var(--text-muted)" }}>{uiText("路径层级", "Path Levels", "パス階層")}</span>
                      <div style={{ display: "flex", gap: 4 }}>
                        {[1, 2, 3].map(n => (
                          <button key={n}
                            className={`btn btn-xs ${(hc.pathLevels || 2) === n ? "btn-primary" : "btn-secondary"}`}
                            onClick={() => void updateHudConfig({ pathLevels: n })}
                            style={{ fontSize: 11, minWidth: 24 }}
                          >
                            {n}
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>

                  {/* Git Status */}
                  <div style={{ marginBottom: 8 }}>
                    <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>Git Status</span>
                    <div style={{ display: "flex", gap: 14, flexWrap: "wrap", marginTop: 6 }}>
                      {([
                        ["enabled", uiText("显示分支", "Branch", "ブランチ表示"), hc.gitStatus?.enabled],
                        ["showDirty", uiText("未提交标记", "Dirty Mark", "変更あり表示"), hc.gitStatus?.showDirty],
                        ["showAheadBehind", uiText("领先/落后", "Ahead/Behind", "先行/遅延"), hc.gitStatus?.showAheadBehind],
                        ["showFileStats", uiText("文件统计", "File Stats", "ファイル統計"), hc.gitStatus?.showFileStats],
                      ] as [string, string, boolean | undefined][]).map(([key, label, value]) => (
                        <label key={key} style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, cursor: "pointer" }}>
                          <input type="checkbox" checked={value !== false}
                            onChange={e => void updateHudConfig({ gitStatus: { ...hc.gitStatus, [key]: e.target.checked } })} />
                          {label}
                        </label>
                      ))}
                    </div>
                  </div>

                  {/* Display Options */}
                  <div>
                    <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>{uiText("显示选项", "Display", "表示項目")}</span>
                    <div style={{ display: "flex", gap: 14, flexWrap: "wrap", marginTop: 6 }}>
                      {([
                        ["showModel", uiText("模型名", "Model", "モデル名"), hc.display?.showModel],
                        ["showContextBar", uiText("上下文进度条", "Context Bar", "コンテキストバー"), hc.display?.showContextBar],
                        ["showConfigCounts", uiText("配置计数", "Config Counts", "設定数"), hc.display?.showConfigCounts],
                        ["showDuration", uiText("会话时长", "Duration", "継続時間"), hc.display?.showDuration],
                        ["showUsage", uiText("用量限制", "Usage", "使用量"), hc.display?.showUsage],
                        ["usageBarEnabled", uiText("用量进度条", "Usage Bar", "使用量バー"), hc.display?.usageBarEnabled],
                        ["showTokenBreakdown", uiText("Token 明细", "Token Detail", "トークン詳細"), hc.display?.showTokenBreakdown],
                        ["showTools", uiText("工具活动", "Tools", "ツール活動"), hc.display?.showTools],
                        ["showAgents", uiText("Agent 活动", "Agents", "Agent 活動"), hc.display?.showAgents],
                        ["showTodos", uiText("Todo 进度", "Todos", "Todo 進捗"), hc.display?.showTodos],
                      ] as [string, string, boolean | undefined][]).map(([key, label, value]) => (
                        <label key={key} style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, cursor: "pointer" }}>
                          <input type="checkbox" checked={value !== false}
                            onChange={e => void updateHudConfig({ display: { ...hc.display, [key]: e.target.checked } })} />
                          {label}
                        </label>
                      ))}
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {tab === "codex" && tools.find(t => t.id === "codex")?.installed && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {/* Approval Mode */}
            <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("审批模式", "Approval Mode", "承認モード")}</h4>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{uiText("操作确认级别", "Action confirmation level", "操作確認レベル")}</p>
              </div>
              <div style={{ display: "flex", gap: 6 }}>
                {[
                  { value: "suggest", label: uiText("建议", "Suggest", "提案") },
                  { value: "auto-edit", label: uiText("自动编辑", "Auto Edit", "自動編集") },
                  { value: "full-auto", label: uiText("全自动", "Full Auto", "フルオート") },
                ].map(opt => (
                  <button key={opt.value}
                    className={`btn btn-xs ${codexApproval === opt.value ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => { setCodexApproval(opt.value); setCodex("approval_mode", opt.value); }}>
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Reasoning Effort */}
            <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("推理强度", "Reasoning Effort", "推論強度")}</h4>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{uiText("模型推理计算量", "Model reasoning compute", "モデルの推論計算量")}</p>
              </div>
              <div style={{ display: "flex", gap: 6 }}>
                {[
                  { value: "low", label: uiText("低", "Low", "低") },
                  { value: "medium", label: uiText("中", "Medium", "中") },
                  { value: "high", label: uiText("高", "High", "高") },
                  { value: "xhigh", label: uiText("极高", "XHigh", "最高") },
                ].map(opt => (
                  <button key={opt.value}
                    className={`btn btn-xs ${codexReasoning === opt.value ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => { setCodexReasoning(opt.value); setCodex("reasoning_effort", opt.value); }}>
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Disable Response Storage */}
            <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("禁用响应存储", "Disable Response Storage", "応答保存を無効化")}</h4>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{uiText("不保存 API 响应到本地", "Don't save API responses locally", "API 応答をローカルに保存しません")}</p>
              </div>
              <ToggleSwitch
                value={codexDisableStorage}
                onChange={v => { setCodexDisableStorage(v); setCodex("disable_response_storage", String(v)); }}
                labelOn={uiText("已禁用", "Disabled", "無効")}
                labelOff={uiText("已启用", "Enabled", "有効")}
              />
            </div>

            <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("1M 上下文窗口", "1M Context Window", "1M コンテキストウィンドウ")}</h4>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                  {uiText("一键写入 `model_context_window = 1000000`", "Write `model_context_window = 1000000` with one toggle", "`model_context_window = 1000000` をワンタップで書き込みます")}
                </p>
              </div>
              <ToggleSwitch
                value={codexContextWindow1M}
                onChange={v => { setCodexContextWindow1M(v); setCodex("context_window_1m", String(v)); }}
                labelOn={uiText("已开启", "Enabled", "有効")}
                labelOff={uiText("默认", "Default", "既定")}
              />
            </div>
          </div>
        )}

        {tab === "opencode" && tools.find(t => t.id === "opencode")?.installed && (
          <OmoConfigSection />
        )}

        {tab === "openclaw" && tools.find(t => t.id === "openclaw")?.installed && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div className="card" style={{ padding: "16px 18px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginBottom: 14, flexWrap: "wrap" }}>
                <div>
                  <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("OpenClaw 配置面板", "OpenClaw Config Panel", "OpenClaw 設定パネル")}</h4>
                  <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                    {uiText("直接编辑 OpenClaw 的 Provider / 模型 / Agent 建议参数，并同步写回 `~/.openclaw/openclaw.json`。", "Edit OpenClaw provider, model, and agent defaults, then write back to `~/.openclaw/openclaw.json`.", "`~/.openclaw/openclaw.json` に OpenClaw の Provider・モデル・Agent 既定値を書き戻します。")}
                  </p>
                </div>
                <div style={{ display: "flex", gap: 8 }}>
                  <button className="btn btn-secondary btn-sm" onClick={() => void loadOpenClawConfig()} disabled={openClawLoading} style={{ gap: 6 }}>
                    {openClawLoading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
                    {uiText("重新读取", "Reload", "再読み込み")}
                  </button>
                  <button className="btn btn-primary btn-sm" onClick={() => void saveOpenClawConfig()} disabled={openClawSaving} style={{ gap: 6 }}>
                    {openClawSaving ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Download size={14} />}
                    {uiText("保存配置", "Save Config", "設定を保存")}
                  </button>
                </div>
              </div>

              {openClawLoading ? (
                <div className="loading-center" style={{ minHeight: 180 }}>
                  <div className="spinner" />
                </div>
              ) : (
                <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
                  <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 14 }}>
                    <FieldCard
                      label={uiText("接口地址", "Base URL", "Base URL")}
                      input={<input className="input" value={openClawDraft.baseUrl} onChange={(e) => updateOpenClawDraft({ baseUrl: e.target.value })} placeholder="https://api.example.com/v1" />}
                    />
                    <FieldCard
                      label="API Key"
                      input={<input className="input" type="password" value={openClawDraft.apiKey} onChange={(e) => updateOpenClawDraft({ apiKey: e.target.value })} placeholder={uiText("填写 API Key", "Enter API Key", "API Key を入力")} />}
                    />
                    <FieldCard
                      label={uiText("API 协议", "API Protocol", "API プロトコル")}
                      input={
                        <select className="input" value={openClawDraft.apiProtocol} onChange={(e) => updateOpenClawDraft({ apiProtocol: e.target.value as OpenClawApiProtocol })}>
                          {OPENCLAW_PROTOCOL_OPTIONS.map((option) => (
                            <option key={option} value={option}>{option}</option>
                          ))}
                        </select>
                      }
                    />
                    <FieldCard
                      label={uiText("模型别名", "Model Alias", "モデル別名")}
                      input={<input className="input" value={openClawDraft.modelCatalogAlias} onChange={(e) => updateOpenClawDraft({ modelCatalogAlias: e.target.value })} placeholder="Claude Sonnet 4.6" />}
                    />
                    <FieldCard
                      label={uiText("模型 ID", "Model ID", "モデル ID")}
                      input={<input className="input" value={openClawDraft.model} onChange={(e) => updateOpenClawDraft({ model: e.target.value })} placeholder="anthropic/claude-sonnet-4-6" />}
                    />
                    <FieldCard
                      label={uiText("显示名", "Display Name", "表示名")}
                      input={<input className="input" value={openClawDraft.modelName} onChange={(e) => updateOpenClawDraft({ modelName: e.target.value })} placeholder={uiText("可选，默认同模型 ID", "Optional, defaults to model ID", "任意。未入力ならモデル ID を使います")} />}
                    />
                    <FieldCard
                      label={uiText("上下文窗口", "Context Window", "コンテキストウィンドウ")}
                      input={<input className="input" value={openClawDraft.openClawContextWindow} onChange={(e) => updateOpenClawDraft({ openClawContextWindow: e.target.value })} placeholder="1000000" />}
                    />
                    <FieldCard
                      label={uiText("主推荐模型", "Suggested Primary", "推奨プライマリ")}
                      input={<input className="input" value={openClawDraft.suggestedPrimaryModel} onChange={(e) => updateOpenClawDraft({ suggestedPrimaryModel: e.target.value })} placeholder="anthropic/claude-sonnet-4-6" />}
                    />
                    <FieldCard
                      label={uiText("输入成本", "Input Cost", "入力コスト")}
                      input={<input className="input" value={openClawDraft.openClawCostInput} onChange={(e) => updateOpenClawDraft({ openClawCostInput: e.target.value })} placeholder="0.003" />}
                    />
                    <FieldCard
                      label={uiText("输出成本", "Output Cost", "出力コスト")}
                      input={<input className="input" value={openClawDraft.openClawCostOutput} onChange={(e) => updateOpenClawDraft({ openClawCostOutput: e.target.value })} placeholder="0.015" />}
                    />
                  </div>

                  <FieldCard
                    label={uiText("备用模型", "Fallback Models", "フォールバックモデル")}
                    input={<input className="input" value={openClawDraft.suggestedFallbackModels} onChange={(e) => updateOpenClawDraft({ suggestedFallbackModels: e.target.value })} placeholder={uiText("逗号分隔，例如 model-a, model-b", "Comma-separated, e.g. model-a, model-b", "カンマ区切り。例: model-a, model-b")} />}
                  />

                  <div>
                    <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8, color: "var(--text-secondary)" }}>
                      {uiText("生成后的配置预览", "Generated Config Preview", "生成された設定プレビュー")}
                    </div>
                    <pre className="code-block" style={{ margin: 0, whiteSpace: "pre-wrap", maxHeight: 260, overflow: "auto", fontSize: 11 }}>
                      {buildStructuredConfig("openclaw", openClawDraft)}
                    </pre>
                  </div>
                </div>
              )}
            </div>

            <div className="card" style={{ padding: "16px 18px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginBottom: 14, flexWrap: "wrap" }}>
                <div>
                  <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("OpenClaw Daily Memory", "OpenClaw Daily Memory", "OpenClaw Daily Memory")}</h4>
                  <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                    {uiText("搜索 `~/.openclaw` 与已发现项目下 `.openclaw` 目录中的 Daily Memory / Journal / Diary 文本。", "Search Daily Memory, Journal, and Diary text files in `~/.openclaw` and discovered project `.openclaw` directories.", "`~/.openclaw` と検出済みプロジェクトの `.openclaw` 配下にある Daily Memory / Journal / Diary テキストを検索します。")}
                  </p>
                </div>
                <button className="btn btn-secondary btn-sm" onClick={() => void loadOpenClawDailyMemory()} disabled={openClawMemoryLoading} style={{ gap: 6 }}>
                  {openClawMemoryLoading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
                  {uiText("刷新结果", "Refresh", "再読み込み")}
                </button>
              </div>

              <div style={{ display: "flex", gap: 10, marginBottom: 14, flexWrap: "wrap" }}>
                <input
                  className="input"
                  value={openClawMemoryQuery}
                  onChange={(e) => setOpenClawMemoryQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      void loadOpenClawDailyMemory();
                    }
                  }}
                  placeholder={uiText("输入关键词，留空则显示最近记录", "Enter a keyword, or leave empty for recent entries", "キーワードを入力。空欄なら最近の記録を表示")}
                  style={{ flex: "1 1 280px" }}
                />
                <button className="btn btn-primary btn-sm" onClick={() => void loadOpenClawDailyMemory()} disabled={openClawMemoryLoading} style={{ gap: 6 }}>
                  {openClawMemoryLoading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Search size={14} />}
                  {openClawMemoryQuery.trim()
                    ? uiText("搜索", "Search", "検索")
                    : uiText("最近记录", "Recent Entries", "最近の記録")}
                </button>
              </div>

              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: 14 }}>
                <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                  {openClawMemoryLoading && openClawMemoryEntries.length === 0 ? (
                    <div className="loading-center" style={{ minHeight: 180 }}>
                      <div className="spinner" />
                    </div>
                  ) : openClawMemoryEntries.length === 0 ? (
                    <div className="card" style={{ padding: "18px 16px", border: "1px dashed var(--border-color)", background: "var(--bg-secondary)" }}>
                      <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>
                        {uiText("没有匹配结果", "No matching entries", "一致する結果はありません")}
                      </div>
                      <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.6 }}>
                        {uiText("会扫描全局 `~/.openclaw` 与已发现项目目录中的 Daily Memory / Journal / Diary 文件。", "The search scans global `~/.openclaw` and discovered project Daily Memory / Journal / Diary files.", "グローバル `~/.openclaw` と検出済みプロジェクト内の Daily Memory / Journal / Diary を走査します。")}
                      </div>
                    </div>
                  ) : (
                    openClawMemoryEntries.map((entry) => {
                      const active = entry.path === openClawMemorySelectedPath;
                      return (
                        <button
                          key={entry.path}
                          type="button"
                          className="card"
                          onClick={() => void openOpenClawDailyMemoryEntry(entry)}
                          style={{
                            padding: "14px 16px",
                            textAlign: "left",
                            border: active ? "1px solid var(--accent-primary)" : "1px solid var(--border-color)",
                            background: active ? "color-mix(in srgb, var(--accent-primary) 10%, var(--bg-secondary))" : "var(--bg-secondary)",
                            display: "flex",
                            flexDirection: "column",
                            gap: 8,
                          }}
                        >
                          <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "flex-start" }}>
                            <div style={{ minWidth: 0 }}>
                              <div style={{ fontSize: 12, fontWeight: 700, color: "var(--text-primary)", wordBreak: "break-word" }}>{entry.file_name}</div>
                              <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                                {entry.source === "global"
                                  ? uiText("全局", "Global", "グローバル")
                                  : uiText("项目", "Project", "プロジェクト")}
                                {entry.project_name ? ` · ${entry.project_name}` : ""}
                              </div>
                            </div>
                            {entry.modified_at && (
                              <span style={{ fontSize: 10, color: "var(--text-muted)", whiteSpace: "nowrap" }}>{entry.modified_at}</span>
                            )}
                          </div>
                          <div style={{ fontSize: 11, color: "var(--text-secondary)", lineHeight: 1.6 }}>{entry.preview}</div>
                          <div style={{ fontSize: 10, color: "var(--text-muted)", wordBreak: "break-all" }}>{entry.path}</div>
                        </button>
                      );
                    })
                  )}
                </div>

                <div className="card" style={{ padding: "14px 16px", minHeight: 280, background: "var(--bg-secondary)" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "center", marginBottom: 10, flexWrap: "wrap" }}>
                    <div style={{ fontSize: 12, fontWeight: 700 }}>
                      {uiText("全文预览", "Full Content", "全文プレビュー")}
                    </div>
                    {openClawMemorySelectedPath && (
                      <div style={{ fontSize: 10, color: "var(--text-muted)", wordBreak: "break-all" }}>{openClawMemorySelectedPath}</div>
                    )}
                  </div>

                  {openClawMemoryLoadingContent ? (
                    <div className="loading-center" style={{ minHeight: 220 }}>
                      <div className="spinner" />
                    </div>
                  ) : openClawMemoryContent ? (
                    <pre className="code-block" style={{ margin: 0, whiteSpace: "pre-wrap", maxHeight: 420, overflow: "auto", fontSize: 11 }}>
                      {openClawMemoryContent}
                    </pre>
                  ) : (
                    <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.7 }}>
                      {uiText("选择左侧结果以查看 Daily Memory 全文。", "Select an entry on the left to inspect the full Daily Memory content.", "左側の結果を選択すると Daily Memory の全文を表示します。")}
                    </div>
                  )}
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function FieldCard({ label, input }: { label: string; input: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <label className="field-label">{label}</label>
      {input}
    </div>
  );
}

function ToggleSwitch({ value, onChange, labelOn, labelOff }: { value: boolean; onChange: (v: boolean) => void; labelOn: string; labelOff: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
      <button className={`toggle toggle-sm ${value ? "on" : "off"}`} onClick={() => onChange(!value)}>
        <span className="toggle-knob" />
      </button>
      <span style={{ fontSize: 12, color: value ? "var(--success)" : "var(--text-muted)", fontWeight: 500 }}>
        {value ? labelOn : labelOff}
      </span>
    </div>
  );
}
