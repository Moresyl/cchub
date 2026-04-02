import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Terminal, Code, Download, RefreshCw } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import type { DetectedTool } from "../types/skills";
import { fetchVisibleApps, type ManagedAppId } from "../lib/appPreferences";

type ToolTab = "claude" | "codex";

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

  const visibleTabs = (["claude", "codex"] as ToolTab[]).filter((id) => visibleApps.includes(id));

  useEffect(() => { loadData(); }, []);
  useEffect(() => {
    if (visibleTabs.length === 0) return;
    if (!visibleTabs.includes(tab)) setTab(visibleTabs[0]);
  }, [tab, visibleTabs]);

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
          const Icon = id === "claude" ? Terminal : Code;
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
                `${tab === "claude" ? "Claude Code" : "Codex CLI"} 未安装`,
                `${tab === "claude" ? "Claude Code" : "Codex CLI"} not installed`,
                `${tab === "claude" ? "Claude Code" : "Codex CLI"} は未インストールです`,
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
                <div style={{ borderTop: "1px solid var(--border)", paddingTop: 12, display: "flex", flexDirection: "column", gap: 14 }}>
                  {/* Layout */}
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <div>
                      <span style={{ fontSize: 12, fontWeight: 600 }}>{uiText("布局", "Layout", "レイアウト")}</span>
                    </div>
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

                  {/* Path Levels */}
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <div>
                      <span style={{ fontSize: 12, fontWeight: 600 }}>{uiText("路径层级", "Path Levels", "パス階層")}</span>
                    </div>
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

                  {/* Git Status */}
                  <div>
                    <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>Git Status</span>
                    <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "8px 16px", marginTop: 8 }}>
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
                    <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "8px 16px", marginTop: 8 }}>
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

      </div>
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
