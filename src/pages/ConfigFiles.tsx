/* eslint-disable react-hooks/exhaustive-deps */
import { useCallback, useEffect, useMemo, useRef, useState, lazy, Suspense, startTransition } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileText, RefreshCw, RotateCcw, Save } from "lucide-react";
import ConfigFilesRootTabs from "../components/ConfigFilesRootTabs";
import ConfigFilesTreePanel from "../components/ConfigFilesTreePanel";
import { getLocale, t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import OmoConfigSection from "../components/OmoConfigSection";
import OpenClawConfigSection from "../components/OpenClawConfigSection";
import HermesConfigSection from "../components/HermesConfigSection";
import { fetchVisibleApps, type ManagedAppId } from "../lib/appPreferences";
import { Checkbox } from "../components/ui/checkbox";
import { Button } from "../components/ui/button";
import { useConfigFiles } from "../hooks/queries";
import { isCodexConfigToml, type CodexStructuredConfig } from "../lib/codexConfig";
import { useCodexEditor } from "./config-files/useCodexEditor";
import CodexStructuredFields from "./config-files/CodexStructuredFields";

import CodeEditor from "../components/DeferredCodeEditor";

const MarkdownEditor = lazy(() => import("../components/MarkdownEditor"));

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
  content: string;
  fileRevision: string;
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
  const [needsReload, setNeedsReload] = useState(false);
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
  const [originalCodexApiKey, setOriginalCodexApiKey] = useState("");
  const [codexFileRevision, setCodexFileRevision] = useState<string | null>(null);
  const fileGeneration = useRef(0);
  const saveInFlight = useRef(false);
  const [claudeToggles, setClaudeToggles] = useState<ClaudeConfigToggles | null>(null);
  const [loadingClaudeToggles, setLoadingClaudeToggles] = useState(false);
  const [writingClaudeToggleKey, setWritingClaudeToggleKey] = useState<string | null>(null);
  const {
    data: tree,
    isLoading: loadingTree,
    error: treeError,
    refetch: refetchTree,
  } = useConfigFiles(activeRoot, Boolean(activeRoot));

  const codexEditor = useCodexEditor(
    content,
    setContent,
    `${activeRoot}:${activeFile}`,
    isCodexConfigToml(activeRoot, activeFile),
  );
  const codexStructuredConfig = codexEditor.config;
  const codexValidation = codexEditor.validation;
  const codexBlocked = Boolean(
    codexEditor.error || codexValidation?.errors.length || codexStructuredConfig?.malformedMcpServers,
  );
  const hasChanges =
    codexEditor.hasPendingDraft ||
    content !== originalContent ||
    (isCodexConfigToml(activeRoot, activeFile) && codexApiKey !== originalCodexApiKey);
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

  const openFile = useCallback(
    async (path: string) => {
      codexEditor.reset();
      const generation = ++fileGeneration.current;
      setLoadingFile(true);
      setActiveFile(path);
      setContent("");
      setOriginalContent("");
      setCodexApiKey("");
      setOriginalCodexApiKey("");
      setCodexFileRevision(null);
      setNeedsReload(false);
      setWritingClaudeToggleKey(null);
      try {
        const nextStructuredCodex = isCodexConfigToml(activeRoot, path);
        const nextClaudeQuickToggleFile = activeRoot === "claude" && /[\\/]settings\.local\.json$/i.test(path);
        setLoadingClaudeToggles(nextClaudeQuickToggleFile);
        const [nextContent, nextCodexStructured, nextClaudeToggles] = await Promise.all([
          nextStructuredCodex ? Promise.resolve("") : invoke<string>("read_config_file_content", { path }),
          nextStructuredCodex
            ? invoke<CodexStructuredBackendConfig>("read_codex_toml_structured", { path })
            : Promise.resolve(null),
          nextClaudeQuickToggleFile
            ? invoke<ClaudeConfigToggles>("read_claude_config_toggles").catch(() => null)
            : Promise.resolve(null),
        ]);
        if (fileGeneration.current !== generation) return;
        startTransition(() => {
          const source = nextCodexStructured?.content ?? nextContent;
          setContent(source);
          setOriginalContent(source);
          setCodexApiKey(nextCodexStructured?.apiKey || "");
          setOriginalCodexApiKey(nextCodexStructured?.apiKey || "");
          setCodexFileRevision(nextCodexStructured?.fileRevision ?? null);
          setClaudeToggles(nextClaudeToggles);
        });
      } catch (error) {
        if (fileGeneration.current !== generation) return;
        console.error(error);
        showToast("error", String(error));
        setContent("");
        setOriginalContent("");
        setCodexApiKey("");
        setClaudeToggles(null);
        setActiveFile(null);
      } finally {
        if (fileGeneration.current === generation) {
          setLoadingFile(false);
          setLoadingClaudeToggles(false);
        }
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
    if (hasChanges) return;
    if (!visibleRoots.some((root) => root.id === activeRoot && root.exists)) {
      const firstExisting = visibleRoots.find((root) => root.exists);
      setActiveRoot(firstExisting?.id || visibleRoots[0].id);
    }
  }, [activeRoot, visibleRoots, hasChanges]);

  useEffect(() => {
    return () => {
      fileGeneration.current++;
    };
  }, []);
  useEffect(() => {
    fileGeneration.current++;
    codexEditor.reset();
    setActiveFile(null);
    setContent("");
    setOriginalContent("");
    setCodexApiKey("");
    setOriginalCodexApiKey("");
    setCodexFileRevision(null);
    setNeedsReload(false);
    setLoadingFile(false);
    setWritingClaudeToggleKey(null);
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
  }, [
    activeFile,
    hasChanges,
    saving,
    content,
    codexApiKey,
    codexFileRevision,
    codexBlocked,
    writingClaudeToggleKey,
    needsReload,
  ]);

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
      if (!hasChanges) setActiveRoot(currentRoot?.id || firstExisting?.id || "");
    } catch (error) {
      console.error(error);
      showToast("error", String(error));
    } finally {
      setLoading(false);
    }
  }

  async function saveFile() {
    if (!activeFile || loadingFile || saveInFlight.current || codexBlocked || needsReload) return;
    const generation = fileGeneration.current;
    saveInFlight.current = true;
    setSaving(true);
    try {
      if (structuredCodexFile) {
        if (!codexFileRevision) throw new Error(zh ? "请重新加载配置后保存" : "Reload the configuration before saving");
        const written = await invoke<{ content: string; fileRevision: string }>("write_codex_toml_structured", {
          path: activeFile,
          rawToml: content,
          expectedRevision: codexFileRevision,
          apiKey: codexApiKey,
        });
        if (fileGeneration.current !== generation) return;
        setContent((current) => (current === content ? written.content : current));
        setOriginalContent(written.content);
        setOriginalCodexApiKey(codexApiKey);
        setCodexFileRevision(written.fileRevision);
      } else {
        await invoke("write_config_file_content", { path: activeFile, content });
        if (fileGeneration.current !== generation) return;
        setOriginalContent(content);
        if (claudeQuickToggleFile) {
          try {
            const refreshed = await invoke<ClaudeConfigToggles>("read_claude_config_toggles");
            if (fileGeneration.current === generation) setClaudeToggles(refreshed);
          } catch (error) {
            if (fileGeneration.current === generation) setNeedsReload(true);
            throw error;
          }
        }
      }
      showToast("success", zh ? "已保存" : "Saved");
    } catch (error) {
      if (fileGeneration.current !== generation) return;
      console.error(error);
      showToast("error", String(error));
    } finally {
      saveInFlight.current = false;
      setSaving(false);
    }
  }

  function updateCodexConfig(patch: Partial<CodexStructuredConfig>) {
    try {
      codexEditor.update(patch);
    } catch (error) {
      showToast("error", String(error));
    }
  }

  function repairCodexConfig() {
    try {
      codexEditor.addMcpTable();
      showToast("success", zh ? "MCP 表已就绪" : "MCP table is ready");
    } catch (error) {
      showToast("error", String(error));
    }
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
    if (saveInFlight.current || loadingFile || needsReload || !claudeToggles) return;
    if (!activeFile || hasChanges) {
      showToast("error", zh ? "请先保存或还原当前修改" : "Save or revert current edits first");
      return;
    }

    saveInFlight.current = true;
    setWritingClaudeToggleKey(key);
    const generation = fileGeneration.current;
    try {
      const nextToggles = await invoke<ClaudeConfigToggles>("write_claude_config_toggle", { key, enabled });
      const nextContent = await invoke<string>("read_config_file_content", { path: activeFile });
      if (fileGeneration.current !== generation) return;
      startTransition(() => {
        setClaudeToggles(nextToggles);
        setContent((current) => (current === content ? nextContent : current));
        setOriginalContent(nextContent);
      });
      showToast("success", zh ? "Claude 快捷开关已更新" : "Claude quick toggle updated");
    } catch (error) {
      if (fileGeneration.current !== generation) return;
      setNeedsReload(true);
      console.error(error);
      showToast("error", String(error));
    } finally {
      saveInFlight.current = false;
      if (fileGeneration.current === generation) setWritingClaudeToggleKey(null);
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
          <Button
            variant="secondary"
            onClick={() => {
              void loadRoots();
              void refetchTree();
            }}
          >
            <RefreshCw size={14} />
            {i.common.refresh}
          </Button>
          <Button
            onClick={saveFile}
            disabled={
              !activeFile ||
              !hasChanges ||
              saving ||
              loadingFile ||
              !!writingClaudeToggleKey ||
              codexBlocked ||
              needsReload
            }
          >
            <Save size={14} />
            {i.common.save}
          </Button>
        </div>
      </div>

      <ConfigFilesRootTabs roots={visibleRoots} activeRoot={activeRoot} onSelectRoot={requestSwitchRoot} />

      {activeRoot === "opencode" && <OmoConfigSection />}
      {activeRoot === "openclaw" && <OpenClawConfigSection />}
      {activeRoot === "hermes" && <HermesConfigSection />}

      <div className="config-files-workspace">
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
              <Button
                variant="secondary"
                onClick={() => {
                  codexEditor.reset();
                  setContent(originalContent);
                  setCodexApiKey(originalCodexApiKey);
                }}
                disabled={!hasChanges}
              >
                <RotateCcw size={14} />
                {i.configFiles.revert}
              </Button>
            )}
          </div>

          <div style={{ flex: 1, minHeight: 0, overflow: "auto", padding: 16 }}>
            {needsReload && activeFile && (
              <div role="alert" className="card" style={{ padding: 12, marginBottom: 16, fontSize: 12 }}>
                {zh
                  ? "文件操作后未能重新读取配置。请重新加载并核对磁盘内容，再继续修改。当前草稿已保留。"
                  : "The configuration could not be re-read after the file operation. Reload and review the file before continuing. Your draft has been retained."}
                <Button variant="secondary" onClick={() => requestOpenFile(activeFile)} style={{ marginTop: 8 }}>
                  <RefreshCw size={14} />
                  {zh ? "重新加载配置" : "Reload configuration"}
                </Button>
              </div>
            )}
            {codexEditor.error && !loadingFile && (
              <div
                role="alert"
                className="card"
                style={{ padding: 12, marginBottom: 16, fontSize: 12, borderColor: "var(--danger)" }}
              >
                {zh
                  ? "结构化字段暂不可用，请在下方修正原始配置。草稿已保留。"
                  : "Correct the raw configuration below to resume structured editing. Your draft has been retained."}
                <div style={{ marginTop: 6 }}>{codexEditor.error}</div>
              </div>
            )}
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
                <CodexStructuredFields
                  zh={zh}
                  config={codexStructuredConfig}
                  validation={codexValidation}
                  onPatch={updateCodexConfig}
                  onAddMcp={repairCodexConfig}
                  onContextWindow1M={toggleCodexContextWindow1M}
                  apiKey={codexApiKey}
                  onApiKeyChange={setCodexApiKey}
                  invalidContextWindow={codexEditor.invalidContextWindow}
                  invalidCompactLimit={codexEditor.invalidCompactLimit}
                />

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

                  {!claudeToggles && (
                    <p role="status" style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 12 }}>
                      {zh
                        ? "快捷开关暂不可用，请在下方修正原始 JSON 配置后保存。"
                        : "Quick toggles are unavailable. Correct and save the raw JSON below."}
                    </p>
                  )}
                  <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
                    {[
                      {
                        key: "hideAttribution",
                        label: zh ? "隐藏署名" : "Hide Attribution",
                        checked: claudeToggles?.hideAttribution ?? false,
                      },
                      {
                        key: "enableTeammates",
                        label: zh ? "启用团队协作" : "Enable Teammates",
                        checked: claudeToggles?.enableTeammates ?? false,
                      },
                      {
                        key: "maxThinkingTokens",
                        label: zh
                          ? `深度思考 (${claudeToggles?.maxThinkingTokensValue || "32000"} tokens)`
                          : `Max Thinking Tokens (${claudeToggles?.maxThinkingTokensValue || "32000"})`,
                        checked: claudeToggles?.maxThinkingTokens ?? false,
                      },
                      {
                        key: "enableToolSearch",
                        label: zh ? "启用工具搜索" : "Enable Tool Search",
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
                        <Checkbox
                          checked={toggle.checked}
                          disabled={
                            hasChanges ||
                            loadingClaudeToggles ||
                            saving ||
                            !!writingClaudeToggleKey ||
                            needsReload ||
                            !claudeToggles
                          }
                          onCheckedChange={(checked) => void handleClaudeQuickToggle(toggle.key, checked === true)}
                        />
                        <span style={{ fontSize: 12 }}>{toggle.label}</span>
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
