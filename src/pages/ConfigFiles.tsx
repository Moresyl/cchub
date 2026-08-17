/* eslint-disable react-hooks/exhaustive-deps */
import { useCallback, useEffect, useMemo, useState, lazy, Suspense, startTransition } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Eye, EyeOff, FileText, RefreshCw, RotateCcw, Save } from "lucide-react";
import ConfigFilesRootTabs from "../components/ConfigFilesRootTabs";
import ConfigFilesTreePanel from "../components/ConfigFilesTreePanel";
import { getLocale, t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import OmoConfigSection from "../components/OmoConfigSection";
import OpenClawConfigSection from "../components/OpenClawConfigSection";
import HermesConfigSection from "../components/HermesConfigSection";
import { fetchVisibleApps, type ManagedAppId } from "../lib/appPreferences";
import { useConfigFiles } from "../hooks/queries";
import {
  isCodexConfigToml,
  parseCodexStructuredConfig,
  repairCodexConfigContent,
  updateCodexStructuredContent,
  validateCodexStructuredConfig,
} from "../lib/codexConfig";

const MarkdownEditor = lazy(() => import("../components/MarkdownEditor"));
const CodeEditor = lazy(() => import("../components/CodeEditor"));

interface ConfigRoot {
  id: string;
  name: string;
  path: string;
  exists: boolean;
}

interface ClaudeConfigToggles {
  hideAttribution: boolean;
  enableTeammates: boolean;
  maxThinkingTokens: boolean;
  maxThinkingTokensValue: string;
  enableToolSearch: boolean;
}

interface CodexStructuredBackendConfig {
  modelProvider: string;
  providerLabel: string;
  baseUrl: string;
  wireApi: string;
  model: string;
  reasoningEffort: string;
  personality: string;
  disableResponseStorage: boolean;
  modelContextWindow: string;
  modelAutoCompactTokenLimit: string;
  apiKey: string;
  mcpServers: string[];
  malformedMcpServers: boolean;
}

type EditorLanguage = "json" | "markdown" | "yaml" | "toml" | "text";
type PendingAction = { type: "openFile"; path: string } | { type: "switchRoot"; rootId: string } | null;

function detectLanguage(path: string): EditorLanguage {
  const lower = path.toLowerCase();
  if (lower.endsWith(".json")) return "json";
  if (lower.endsWith(".toml")) return "toml";
  if (lower.endsWith(".yaml") || lower.endsWith(".yml")) return "yaml";
  if (lower.endsWith(".md") || lower.endsWith(".mdx") || lower.endsWith(".markdown")) return "markdown";
  return "text";
}

export default function ConfigFiles() {
  const i = t();
  const locale = getLocale();
  const zh = locale === "zh";
  const [roots, setRoots] = useState<ConfigRoot[]>([]);
  const [activeRoot, setActiveRoot] = useState("");
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [activeFile, setActiveFile] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const [originalContent, setOriginalContent] = useState("");
  const [loading, setLoading] = useState(true);
  const [loadingFile, setLoadingFile] = useState(false);
  const [saving, setSaving] = useState(false);
  const [pendingAction, setPendingAction] = useState<PendingAction>(null);
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>([
    "claude",
    "codex",
    "gemini",
    "grokbuild",
    "opencode",
    "openclaw",
    "hermes",
  ]);
  const [codexApiKey, setCodexApiKey] = useState("");
  const [showCodexApiKey, setShowCodexApiKey] = useState(false);
  const [claudeToggles, setClaudeToggles] = useState<ClaudeConfigToggles | null>(null);
  const [loadingClaudeToggles, setLoadingClaudeToggles] = useState(false);
  const [writingClaudeToggleKey, setWritingClaudeToggleKey] = useState<string | null>(null);
  const {
    data: tree,
    isLoading: loadingTree,
    error: treeError,
    refetch: refetchTree,
  } = useConfigFiles(activeRoot, Boolean(activeRoot));

  const hasChanges = content !== originalContent;
  const visibleRoots = useMemo(
    () => roots.filter((root) => visibleApps.includes(root.id as ManagedAppId)),
    [roots, visibleApps],
  );
  const activeRootMeta = useMemo(
    () => visibleRoots.find((root) => root.id === activeRoot) || null,
    [visibleRoots, activeRoot],
  );
  const activeLanguage = activeFile ? detectLanguage(activeFile) : "text";
  const structuredCodexFile = useMemo(() => isCodexConfigToml(activeRoot, activeFile), [activeRoot, activeFile]);
  const claudeQuickToggleFile = useMemo(
    () => activeRoot === "claude" && Boolean(activeFile && /[\\/]settings\.local\.json$/i.test(activeFile)),
    [activeRoot, activeFile],
  );
  const codexStructuredConfig = useMemo(
    () => (structuredCodexFile ? parseCodexStructuredConfig(content) : null),
    [structuredCodexFile, content],
  );
  const codexValidation = useMemo(
    () => (codexStructuredConfig ? validateCodexStructuredConfig(codexStructuredConfig) : null),
    [codexStructuredConfig],
  );

  const openFile = useCallback(
    async (path: string) => {
      setLoadingFile(true);
      setActiveFile(path);
      try {
        const nextStructuredCodex = isCodexConfigToml(activeRoot, path);
        const nextClaudeQuickToggleFile = activeRoot === "claude" && /[\\/]settings\.local\.json$/i.test(path);
        setLoadingClaudeToggles(nextClaudeQuickToggleFile);
        const [nextContent, nextCodexStructured, nextClaudeToggles] = await Promise.all([
          invoke<string>("read_config_file_content", { path }),
          nextStructuredCodex
            ? invoke<CodexStructuredBackendConfig>("read_codex_toml_structured", { path })
            : Promise.resolve(null),
          nextClaudeQuickToggleFile ? invoke<ClaudeConfigToggles>("read_claude_config_toggles") : Promise.resolve(null),
        ]);
        startTransition(() => {
          setContent(nextContent);
          setOriginalContent(nextContent);
          setCodexApiKey(nextCodexStructured?.apiKey || "");
          setShowCodexApiKey(false);
          setClaudeToggles(nextClaudeToggles);
        });
      } catch (error) {
        console.error(error);
        showToast("error", String(error));
        setContent("");
        setOriginalContent("");
        setCodexApiKey("");
        setClaudeToggles(null);
      } finally {
        setLoadingFile(false);
        setLoadingClaudeToggles(false);
      }
    },
    [activeRoot],
  );

  const toggleExpand = useCallback((path: string) => {
    setExpanded((current) => ({ ...current, [path]: !current[path] }));
  }, []);

  const requestOpenFile = useCallback(
    (path: string) => {
      if (hasChanges) {
        setPendingAction({ type: "openFile", path });
        return;
      }
      void openFile(path);
    },
    [hasChanges, openFile],
  );

  const requestSwitchRoot = useCallback(
    (rootId: string) => {
      if (hasChanges) {
        setPendingAction({ type: "switchRoot", rootId });
        return;
      }
      setActiveRoot(rootId);
    },
    [hasChanges],
  );

  useEffect(() => {
    loadRoots();
  }, []);
  useEffect(() => {
    if (visibleRoots.length === 0) return;
    if (!visibleRoots.some((root) => root.id === activeRoot && root.exists)) {
      const firstExisting = visibleRoots.find((root) => root.exists);
      setActiveRoot(firstExisting?.id || visibleRoots[0].id);
    }
  }, [activeRoot, visibleRoots]);

  useEffect(() => {
    setActiveFile(null);
    setContent("");
    setOriginalContent("");
    setCodexApiKey("");
    setShowCodexApiKey(false);
    setClaudeToggles(null);
  }, [activeRoot]);
  useEffect(() => {
    if (tree) {
      setExpanded({ [tree.path]: true });
    }
  }, [tree]);
  useEffect(() => {
    if (treeError) {
      console.error(treeError);
      showToast("error", String(treeError));
    }
  }, [treeError]);
  useEffect(() => {
    const handleSave = () => {
      if (activeFile && hasChanges && !saving) {
        void saveFile();
      }
    };
    window.addEventListener("cchub-shortcut-save", handleSave);
    return () => window.removeEventListener("cchub-shortcut-save", handleSave);
  }, [activeFile, hasChanges, saving, content]);

  async function loadRoots() {
    setLoading(true);
    try {
      const [result, nextVisibleApps] = await Promise.all([
        invoke<ConfigRoot[]>("get_config_roots"),
        fetchVisibleApps(),
      ]);
      setRoots(result);
      setVisibleApps(nextVisibleApps);
      const currentRoot = result.find(
        (root) => root.id === activeRoot && root.exists && nextVisibleApps.includes(root.id as ManagedAppId),
      );
      const firstExisting = result.find((root) => root.exists && nextVisibleApps.includes(root.id as ManagedAppId));
      setActiveRoot(currentRoot?.id || firstExisting?.id || "");
    } catch (error) {
      console.error(error);
      showToast("error", String(error));
    } finally {
      setLoading(false);
    }
  }

  async function saveFile() {
    if (!activeFile) return;
    setSaving(true);
    try {
      if (structuredCodexFile) {
        const nextStructured = parseCodexStructuredConfig(content);
        const writtenToml = await invoke<string>("write_codex_toml_structured", {
          path: activeFile,
          rawToml: content,
          config: {
            ...nextStructured,
            apiKey: codexApiKey,
          },
        });
        setContent(writtenToml);
        setOriginalContent(writtenToml);
      } else {
        await invoke("write_config_file_content", { path: activeFile, content });
        setOriginalContent(content);
      }
      showToast("success", zh ? "已保存" : "Saved");
    } catch (error) {
      console.error(error);
      showToast("error", String(error));
    } finally {
      setSaving(false);
    }
  }

  function updateCodexConfig(patch: Parameters<typeof updateCodexStructuredContent>[1]) {
    setContent((current) => updateCodexStructuredContent(current, patch));
  }

  function repairCodexConfig() {
    setContent((current) => repairCodexConfigContent(current));
    showToast("success", zh ? "已修复 Codex MCP 表结构" : "Codex MCP table repaired");
  }

  function toggleCodexContextWindow1M(enabled: boolean) {
    if (!codexStructuredConfig) return;
    if (enabled) {
      updateCodexConfig({
        modelContextWindow: "1000000",
        modelAutoCompactTokenLimit: codexStructuredConfig.modelAutoCompactTokenLimit.trim() || "900000",
      });
      return;
    }
    updateCodexConfig({
      modelContextWindow: "",
      modelAutoCompactTokenLimit: "",
    });
  }

  async function handleClaudeQuickToggle(key: string, enabled: boolean) {
    if (!activeFile || hasChanges) {
      showToast("error", zh ? "请先保存或还原当前修改" : "Save or revert current edits first");
      return;
    }

    setWritingClaudeToggleKey(key);
    try {
      const nextToggles = await invoke<ClaudeConfigToggles>("write_claude_config_toggle", { key, enabled });
      const nextContent = await invoke<string>("read_config_file_content", { path: activeFile });
      startTransition(() => {
        setClaudeToggles(nextToggles);
        setContent(nextContent);
        setOriginalContent(nextContent);
      });
      showToast("success", zh ? "Claude 快捷开关已更新" : "Claude quick toggle updated");
    } catch (error) {
      console.error(error);
      showToast("error", String(error));
    } finally {
      setWritingClaudeToggleKey(null);
    }
  }

  function handleConfirmPendingAction() {
    const action = pendingAction;
    setPendingAction(null);
    if (!action) return;
    if (action.type === "openFile") {
      void openFile(action.path);
      return;
    }
    setActiveRoot(action.rootId);
  }

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>{i.configFiles.loading}</span>
      </div>
    );
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.configFiles.title}</h2>
          <p className="page-subtitle">{i.configFiles.subtitle}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => {
              void loadRoots();
              void refetchTree();
            }}
          >
            <RefreshCw size={14} />
            {i.common.refresh}
          </button>
          <button className="btn btn-primary btn-sm" onClick={saveFile} disabled={!activeFile || !hasChanges || saving}>
            <Save size={14} />
            {i.common.save}
          </button>
        </div>
      </div>

      <ConfigFilesRootTabs roots={visibleRoots} activeRoot={activeRoot} onSelectRoot={requestSwitchRoot} />

      {activeRoot === "opencode" && <OmoConfigSection />}
      {activeRoot === "openclaw" && <OpenClawConfigSection />}
      {activeRoot === "hermes" && <HermesConfigSection />}

      <div style={{ flex: 1, minHeight: 0, display: "grid", gridTemplateColumns: "320px minmax(0, 1fr)", gap: 16 }}>
        <ConfigFilesTreePanel
          title={i.configFiles.folders}
          rootPath={activeRootMeta?.path || i.common.na}
          loading={loadingTree}
          tree={tree}
          activeFile={activeFile}
          expanded={expanded}
          noRootLabel={i.configFiles.noRoot}
          noRootTip={i.configFiles.noRootTip}
          onToggleExpand={toggleExpand}
          onOpenFile={requestOpenFile}
        />

        <div className="card" style={{ minHeight: 0, overflow: "hidden", display: "flex", flexDirection: "column" }}>
          <div
            style={{
              padding: "14px 16px",
              borderBottom: "1px solid var(--border-default)",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: 12,
            }}
          >
            <div style={{ minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span
                  style={{
                    fontSize: 13,
                    fontWeight: 700,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {activeFile ? activeFile.split(/[/\\]/).pop() : i.configFiles.selectFile}
                </span>
                {hasChanges && <span className="badge badge-warning">{i.configFiles.unsaved}</span>}
              </div>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  marginTop: 4,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {activeFile || i.configFiles.selectTip}
              </div>
            </div>
            {activeFile && (
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => setContent(originalContent)}
                disabled={!hasChanges}
              >
                <RotateCcw size={14} />
                {i.configFiles.revert}
              </button>
            )}
          </div>

          <div style={{ flex: 1, minHeight: 0, overflow: "auto", padding: 16 }}>
            {!activeFile ? (
              <div className="empty-state" style={{ minHeight: "100%" }}>
                <div className="empty-icon">
                  <FileText size={28} style={{ color: "var(--text-muted)" }} />
                </div>
                <p style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)" }}>
                  {i.configFiles.selectFile}
                </p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 8, maxWidth: 260 }}>
                  {i.configFiles.selectTip}
                </p>
              </div>
            ) : loadingFile ? (
              <div className="loading-center" style={{ height: "100%" }}>
                <div className="spinner" />
              </div>
            ) : structuredCodexFile && codexStructuredConfig ? (
              <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
                <div className="section-card" style={{ padding: 16 }}>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      gap: 12,
                      marginBottom: 14,
                      flexWrap: "wrap",
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 14, fontWeight: 700 }}>
                        {zh ? "Codex 结构化编辑" : "Codex Structured Editor"}
                      </div>
                      <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
                        {zh
                          ? "字段级编辑会直接同步到下方的 config.toml，原始内容仍可继续手动修改。"
                          : "Field edits write back into the TOML below, while preserving raw editing for advanced cases."}
                      </div>
                    </div>
                    <button className="btn btn-secondary btn-sm" onClick={repairCodexConfig} style={{ gap: 6 }}>
                      <RefreshCw size={14} />
                      {zh ? "修复 MCP 表" : "Repair MCP Table"}
                    </button>
                  </div>

                  {codexValidation && (codexValidation.errors.length > 0 || codexValidation.warnings.length > 0) && (
                    <div style={{ display: "flex", flexDirection: "column", gap: 8, marginBottom: 14 }}>
                      {codexValidation.errors.map((message) => (
                        <div
                          key={`error:${message}`}
                          className="card"
                          style={{
                            padding: "10px 12px",
                            borderColor: "var(--danger)",
                            background: "color-mix(in srgb, var(--danger) 8%, var(--bg-card))",
                            fontSize: 12,
                          }}
                        >
                          {zh ? `错误：${message}` : `Error: ${message}`}
                        </div>
                      ))}
                      {codexValidation.warnings.map((message) => (
                        <div
                          key={`warning:${message}`}
                          className="card"
                          style={{
                            padding: "10px 12px",
                            borderColor: "var(--warning)",
                            background: "color-mix(in srgb, var(--warning) 10%, var(--bg-card))",
                            fontSize: 12,
                          }}
                        >
                          {zh ? `提示：${message}` : `Warning: ${message}`}
                        </div>
                      ))}
                    </div>
                  )}

                  <div
                    style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 14 }}
                  >
                    <div>
                      <label className="field-label">{zh ? "模型 Provider" : "Model Provider"}</label>
                      <input
                        className="input"
                        value={codexStructuredConfig.modelProvider}
                        onChange={(event) => updateCodexConfig({ modelProvider: event.target.value || "custom" })}
                        placeholder="custom"
                      />
                    </div>
                    <div>
                      <label className="field-label">{zh ? "Provider 显示名" : "Provider Label"}</label>
                      <input
                        className="input"
                        value={codexStructuredConfig.providerLabel}
                        onChange={(event) => updateCodexConfig({ providerLabel: event.target.value })}
                        placeholder="custom"
                      />
                    </div>
                    <div>
                      <label className="field-label">{zh ? "模型 ID" : "Model ID"}</label>
                      <input
                        className="input"
                        value={codexStructuredConfig.model}
                        onChange={(event) => updateCodexConfig({ model: event.target.value })}
                        placeholder="gpt-5.6-sol"
                      />
                    </div>
                    <div>
                      <label className="field-label">{zh ? "Base URL" : "Base URL"}</label>
                      <input
                        className="input"
                        value={codexStructuredConfig.baseUrl}
                        onChange={(event) => updateCodexConfig({ baseUrl: event.target.value })}
                        placeholder="https://api.example.com/v1"
                      />
                    </div>
                    <div>
                      <label className="field-label">{zh ? "API Key" : "API Key"}</label>
                      <div style={{ position: "relative" }}>
                        <input
                          className="input"
                          type={showCodexApiKey ? "text" : "password"}
                          value={codexApiKey}
                          onChange={(event) => setCodexApiKey(event.target.value)}
                          placeholder="sk-..."
                          style={{ paddingRight: 40 }}
                        />
                        <button
                          className="btn btn-ghost btn-icon-sm"
                          type="button"
                          onClick={() => setShowCodexApiKey((current) => !current)}
                          style={{ position: "absolute", right: 4, top: "50%", transform: "translateY(-50%)" }}
                        >
                          {showCodexApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
                        </button>
                      </div>
                    </div>
                    <div>
                      <label className="field-label">{zh ? "推理强度" : "Reasoning Effort"}</label>
                      <select
                        className="input"
                        value={codexStructuredConfig.reasoningEffort}
                        onChange={(event) => updateCodexConfig({ reasoningEffort: event.target.value })}
                      >
                        {["low", "medium", "high", "xhigh"].map((option) => (
                          <option key={option} value={option}>
                            {option}
                          </option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="field-label">{zh ? "Wire API" : "Wire API"}</label>
                      <select
                        className="input"
                        value={codexStructuredConfig.wireApi}
                        onChange={(event) => updateCodexConfig({ wireApi: event.target.value })}
                      >
                        {["responses", "chat"].map((option) => (
                          <option key={option} value={option}>
                            {option}
                          </option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="field-label">{zh ? "执行人格" : "Personality"}</label>
                      <select
                        className="input"
                        value={codexStructuredConfig.personality}
                        onChange={(event) => updateCodexConfig({ personality: event.target.value })}
                      >
                        {["pragmatic", "full-auto", "auto-edit", "explain"].map((option) => (
                          <option key={option} value={option}>
                            {option}
                          </option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="field-label">{zh ? "上下文窗口" : "Context Window"}</label>
                      <input
                        className="input"
                        value={codexStructuredConfig.modelContextWindow}
                        onChange={(event) =>
                          updateCodexConfig({ modelContextWindow: event.target.value.replace(/[,_\s]/g, "") })
                        }
                        placeholder="1000000"
                      />
                    </div>
                    <div>
                      <label className="field-label">{zh ? "自动压缩阈值" : "Auto Compact Limit"}</label>
                      <input
                        className="input"
                        value={codexStructuredConfig.modelAutoCompactTokenLimit}
                        onChange={(event) =>
                          updateCodexConfig({ modelAutoCompactTokenLimit: event.target.value.replace(/[,_\s]/g, "") })
                        }
                        placeholder="900000"
                      />
                    </div>
                  </div>

                  <div style={{ display: "flex", alignItems: "center", gap: 18, marginTop: 14, flexWrap: "wrap" }}>
                    <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                      <input
                        type="checkbox"
                        checked={codexStructuredConfig.disableResponseStorage}
                        onChange={(event) => updateCodexConfig({ disableResponseStorage: event.target.checked })}
                      />
                      {zh ? "禁用响应存储" : "Disable Response Storage"}
                    </label>
                    <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                      <input
                        type="checkbox"
                        checked={codexStructuredConfig.modelContextWindow === "1000000"}
                        onChange={(event) => toggleCodexContextWindow1M(event.target.checked)}
                      />
                      {zh ? "1M 上下文窗口" : "1M Context Window"}
                    </label>
                    <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                      {codexStructuredConfig.mcpServers.length > 0
                        ? zh
                          ? `已检测到 ${codexStructuredConfig.mcpServers.length} 个 MCP Server: ${codexStructuredConfig.mcpServers.join(", ")}`
                          : `${codexStructuredConfig.mcpServers.length} MCP server(s): ${codexStructuredConfig.mcpServers.join(", ")}`
                        : zh
                          ? "当前未配置 MCP Server 表项。"
                          : "No MCP server sections detected yet."}
                    </div>
                  </div>
                </div>

                <CodeEditor value={content} onChange={setContent} language={activeLanguage} minHeight={520} />
              </div>
            ) : claudeQuickToggleFile ? (
              <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
                <div className="section-card" style={{ padding: 16 }}>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      gap: 12,
                      marginBottom: 14,
                      flexWrap: "wrap",
                    }}
                  >
                    <div>
                      <div style={{ fontSize: 14, fontWeight: 700 }}>
                        {zh ? "Claude Quick Toggles" : "Claude Quick Toggles"}
                      </div>
                      <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
                        {zh
                          ? "这些快捷开关会直接写入 settings.local.json，并同步刷新下方原始 JSON。"
                          : "These toggles write directly into settings.local.json and refresh the raw JSON below."}
                      </div>
                    </div>
                    {hasChanges && (
                      <span className="badge badge-warning" style={{ fontSize: 10 }}>
                        {zh ? "请先保存当前修改" : "Save current edits first"}
                      </span>
                    )}
                  </div>

                  <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
                    {[
                      {
                        key: "hideAttribution",
                        label: zh ? "Hide Attribution" : "Hide Attribution",
                        checked: claudeToggles?.hideAttribution ?? false,
                      },
                      {
                        key: "enableTeammates",
                        label: zh ? "Enable Teammates" : "Enable Teammates",
                        checked: claudeToggles?.enableTeammates ?? false,
                      },
                      {
                        key: "maxThinkingTokens",
                        label: zh
                          ? `Max Thinking Tokens (${claudeToggles?.maxThinkingTokensValue || "32000"})`
                          : `Max Thinking Tokens (${claudeToggles?.maxThinkingTokensValue || "32000"})`,
                        checked: claudeToggles?.maxThinkingTokens ?? false,
                      },
                      {
                        key: "enableToolSearch",
                        label: zh ? "Enable Tool Search" : "Enable Tool Search",
                        checked: claudeToggles?.enableToolSearch ?? false,
                      },
                    ].map((toggle) => (
                      <label
                        key={toggle.key}
                        className="card"
                        style={{
                          padding: "10px 12px",
                          display: "flex",
                          alignItems: "center",
                          gap: 8,
                          minWidth: 190,
                          background: "var(--bg-elevated)",
                          opacity: hasChanges ? 0.6 : 1,
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={toggle.checked}
                          disabled={hasChanges || loadingClaudeToggles || writingClaudeToggleKey === toggle.key}
                          onChange={(event) => void handleClaudeQuickToggle(toggle.key, event.target.checked)}
                        />
                        <span style={{ fontSize: 13 }}>{toggle.label}</span>
                        {writingClaudeToggleKey === toggle.key && (
                          <div className="spinner" style={{ width: 12, height: 12, marginLeft: "auto" }} />
                        )}
                      </label>
                    ))}
                  </div>
                </div>

                <CodeEditor value={content} onChange={setContent} language={activeLanguage} minHeight={520} />
              </div>
            ) : activeLanguage === "markdown" ? (
              <Suspense
                fallback={
                  <div className="loading-center" style={{ height: "100%" }}>
                    <div className="spinner" />
                  </div>
                }
              >
                <MarkdownEditor value={content} onChange={setContent} minHeight={520} />
              </Suspense>
            ) : (
              <CodeEditor value={content} onChange={setContent} language={activeLanguage} minHeight={520} />
            )}
          </div>
        </div>
      </div>

      <ConfirmDialog
        isOpen={!!pendingAction}
        title={zh ? "未保存的修改" : "Unsaved Changes"}
        message={
          zh
            ? "当前文件有未保存修改，继续操作会丢失这些更改。"
            : "The current file has unsaved changes. Continuing will discard them."
        }
        confirmText={zh ? "继续" : "Continue"}
        cancelText={i.common.cancel}
        variant="info"
        onConfirm={handleConfirmPendingAction}
        onCancel={() => setPendingAction(null)}
      />
    </div>
  );
}
