/* eslint-disable react-hooks/exhaustive-deps */
import { useCallback, useState, useEffect, lazy, Suspense } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Save, RotateCcw, Plus, Trash2, ArrowLeft, Download, X, GitBranch, Upload } from "lucide-react";
import { getLocale, t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import { fetchVisibleApps, type ManagedAppId } from "../lib/appPreferences";
import WorkflowCard from "../components/WorkflowCard";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import EmptyState from "../components/states/EmptyState";
import {
  useDeleteWorkflowMutation,
  useImportWorkflowFileMutation,
  useInstallWorkflowMutation,
  useToggleWorkflowMutation,
  useWriteWorkflowContentMutation,
} from "../hooks/mutations";

const MarkdownEditor = lazy(() => import("../components/MarkdownEditor"));

type WorkflowAppId = Exclude<ManagedAppId, "hermes">;

interface WorkflowFile {
  path: string;
  tool_id: string;
  tool_name: string;
  name: string;
  file_name: string;
  size_bytes: number;
  modified_at: string | null;
  content_preview: string;
  disabled: boolean;
}

interface WorkflowTemplate {
  id: string;
  name_zh: string;
  name_en: string;
  description_zh: string;
  description_en: string;
  category: string;
  content: string;
}

const TOOL_TABS = [
  { id: "all", label_zh: "全部", label_en: "All" },
  { id: "claude", label_zh: "Claude", label_en: "Claude" },
  { id: "codex", label_zh: "Codex", label_en: "Codex" },
  { id: "gemini", label_zh: "Gemini", label_en: "Gemini" },
  { id: "opencode", label_zh: "OpenCode", label_en: "OpenCode" },
  { id: "openclaw", label_zh: "OpenClaw", label_en: "OpenClaw" },
  { id: "hermes", label_zh: "Hermes", label_en: "Hermes" },
];

const TOOL_COLORS: Record<string, string> = {
  claude: "#d97706",
  codex: "#2563eb",
  gemini: "#059669",
  opencode: "#7c3aed",
  openclaw: "#dc2626",
  hermes: "#0f766e",
};

const TOOL_NAMES: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  opencode: "OpenCode",
  openclaw: "OpenClaw",
  hermes: "Hermes",
};

function workflowFileName(path: string) {
  return path.split(/[\\/]/).pop() || path;
}

function workflowDisplayName(fileName: string) {
  return fileName
    .replace(/\.disabled$/, "")
    .replace(/\.md$/, "")
    .replace(/[-_]/g, " ");
}

function workflowPreview(content: string) {
  return content.length > 200 ? `${content.slice(0, 200)}...` : content;
}

function sortWorkflowFiles(files: WorkflowFile[]) {
  return [...files].sort(
    (left, right) => left.tool_id.localeCompare(right.tool_id) || left.name.localeCompare(right.name),
  );
}

function buildWorkflowFile(path: string, toolId: string, content: string): WorkflowFile {
  const fileName = workflowFileName(path);
  return {
    path,
    tool_id: toolId,
    tool_name: TOOL_NAMES[toolId] ?? toolId,
    name: workflowDisplayName(fileName),
    file_name: fileName,
    size_bytes: new Blob([content]).size,
    modified_at: null,
    content_preview: workflowPreview(content),
    disabled: fileName.endsWith(".disabled"),
  };
}

export default function Workflows() {
  const [files, setFiles] = useState<WorkflowFile[]>([]);
  const [templates, setTemplates] = useState<WorkflowTemplate[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState("all");
  const [editingFile, setEditingFile] = useState<WorkflowFile | null>(null);
  const [content, setContent] = useState("");
  const [originalContent, setOriginalContent] = useState("");
  const [loadingContent, setLoadingContent] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [showInstall, setShowInstall] = useState(false);
  const [installTool, setInstallTool] = useState("claude");
  const [installing, setInstalling] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<WorkflowFile | null>(null);
  const [togglingPath, setTogglingPath] = useState<string | null>(null);
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>([
    "claude",
    "codex",
    "gemini",
    "opencode",
    "openclaw",
    "hermes",
  ]);
  const i = t();
  const zh = getLocale() === "zh";
  const writeWorkflowContentMutation = useWriteWorkflowContentMutation();
  const deleteWorkflowMutation = useDeleteWorkflowMutation();
  const toggleWorkflowMutation = useToggleWorkflowMutation();
  const installWorkflowMutation = useInstallWorkflowMutation();
  const importWorkflowFileMutation = useImportWorkflowFileMutation();
  const hasChanges = content !== originalContent;

  useEffect(() => {
    load();
  }, []);
  useEffect(() => {
    const handleEscape = () => {
      if (editingFile) {
        closeEditor();
        return;
      }
      if (showInstall) {
        setShowInstall(false);
      }
    };
    window.addEventListener("cchub-shortcut-escape", handleEscape);
    return () => window.removeEventListener("cchub-shortcut-escape", handleEscape);
  }, [editingFile, showInstall]);
  useEffect(() => {
    const handleSaveShortcut = () => {
      if (editingFile && hasChanges && !saving) {
        void handleSave();
      }
    };
    const handleNewShortcut = () => {
      if (!editingFile) {
        setShowInstall(true);
      }
    };
    window.addEventListener("cchub-shortcut-save", handleSaveShortcut);
    window.addEventListener("cchub-shortcut-new", handleNewShortcut);
    return () => {
      window.removeEventListener("cchub-shortcut-save", handleSaveShortcut);
      window.removeEventListener("cchub-shortcut-new", handleNewShortcut);
    };
  }, [editingFile, hasChanges, saving]);

  async function load() {
    setLoading(true);
    setLoadError(null);
    try {
      const [f, tmpl, nextVisibleApps] = await Promise.all([
        invoke<WorkflowFile[]>("scan_workflows"),
        invoke<WorkflowTemplate[]>("get_workflow_templates"),
        fetchVisibleApps(),
      ]);
      setFiles(f);
      setTemplates(tmpl);
      setVisibleApps(nextVisibleApps);
    } catch (e) {
      console.error(e);
      setLoadError(String(e));
    } finally {
      setLoading(false);
    }
  }

  const openEditor = useCallback(async (file: WorkflowFile) => {
    setEditingFile(file);
    setLoadingContent(true);
    try {
      const c = await invoke<string>("read_workflow_content", { path: file.path });
      setContent(c);
      setOriginalContent(c);
    } catch (e) {
      console.error(e);
      setContent("Failed to load file");
      setOriginalContent("");
    } finally {
      setLoadingContent(false);
    }
  }, []);

  const closeEditor = useCallback(() => {
    setEditingFile(null);
    setContent("");
    setOriginalContent("");
  }, []);

  async function handleSave() {
    if (!editingFile) return;
    setSaving(true);
    try {
      await writeWorkflowContentMutation.mutateAsync({ path: editingFile.path, content });
      setOriginalContent(content);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      showToast("success", zh ? "已保存" : "Saved");
    } catch (e) {
      showToast("error", `${e}`);
    } finally {
      setSaving(false);
    }
  }

  const handleDelete = useCallback(
    async (file: WorkflowFile) => {
      try {
        await deleteWorkflowMutation.mutateAsync({ path: file.path });
        showToast("success", zh ? "已删除" : "Deleted");
        if (editingFile?.path === file.path) closeEditor();
        setFiles((current) => current.filter((item) => item.path !== file.path));
        setConfirmDelete(null);
      } catch (e) {
        showToast("error", `${e}`);
      }
    },
    [deleteWorkflowMutation, editingFile],
  );

  const handleToggle = useCallback(
    async (file: WorkflowFile) => {
      setTogglingPath(file.path);
      try {
        const nextPath = await toggleWorkflowMutation.mutateAsync({ path: file.path, enabled: file.disabled });
        const nextFileName = workflowFileName(nextPath);
        const nextFile = {
          ...file,
          path: nextPath,
          file_name: nextFileName,
          disabled: nextFileName.endsWith(".disabled"),
        };
        setFiles((current) => current.map((item) => (item.path === file.path ? nextFile : item)));
        if (editingFile?.path === file.path) {
          setEditingFile(nextFile);
        }
        showToast("success", zh ? "已更新" : "Updated");
      } catch (e) {
        showToast("error", `${e}`);
      } finally {
        setTogglingPath(null);
      }
    },
    [editingFile?.path, toggleWorkflowMutation, zh],
  );

  async function handleInstall(templateId: string) {
    setInstalling(templateId);
    try {
      const path = await installWorkflowMutation.mutateAsync({ toolId: installTool, templateId });
      const template = templates.find((item) => item.id === templateId);
      if (template) {
        setFiles((current) => sortWorkflowFiles([...current, buildWorkflowFile(path, installTool, template.content)]));
      }
      showToast("success", zh ? "安装成功" : "Installed");
    } catch (e) {
      showToast("error", `${e}`);
    } finally {
      setInstalling(null);
    }
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  const handleOpenEditor = useCallback(
    (file: WorkflowFile) => {
      void openEditor(file);
    },
    [openEditor],
  );

  const handleToggleWorkflow = useCallback(
    (file: WorkflowFile) => {
      void handleToggle(file);
    },
    [handleToggle],
  );

  const handleRequestDelete = useCallback((file: WorkflowFile) => {
    setConfirmDelete(file);
  }, []);

  const visibleToolTabs = TOOL_TABS.filter((tab) => tab.id === "all" || visibleApps.includes(tab.id as ManagedAppId));
  const workflowCapableApps = visibleApps.filter((appId): appId is WorkflowAppId => appId !== "hermes");
  const visibleFiles = files.filter(
    (file) => visibleApps.includes(file.tool_id as ManagedAppId) && file.tool_id !== "hermes",
  );
  const filteredFiles = activeTab === "all" ? visibleFiles : visibleFiles.filter((f) => f.tool_id === activeTab);
  const hermesWorkflowUnsupported = activeTab === "hermes";

  async function handleImport() {
    try {
      const fallbackTool = workflowCapableApps[0] || "claude";
      const toolId = activeTab === "all" ? fallbackTool : activeTab;
      const path = await importWorkflowFileMutation.mutateAsync({ toolId });
      const importedContent = await invoke<string>("read_workflow_content", { path });
      setFiles((current) => sortWorkflowFiles([...current, buildWorkflowFile(path, toolId, importedContent)]));
      showToast("success", i.workflows.importSuccess);
    } catch (e) {
      const msg = String(e);
      if (msg !== "Cancelled") showToast("error", msg);
    }
  }

  const installedIds = new Set(
    visibleFiles
      .filter((f) => f.tool_id === installTool)
      .map((f) => f.file_name.replace(".md", "").replace(".disabled", "")),
  );

  useEffect(() => {
    const firstVisibleTool = workflowCapableApps[0] || "claude";
    if (activeTab !== "all" && !visibleApps.includes(activeTab as ManagedAppId)) {
      setActiveTab("all");
    }
    if (!workflowCapableApps.includes(installTool as WorkflowAppId)) {
      setInstallTool(firstVisibleTool);
    }
  }, [activeTab, installTool, visibleApps, workflowCapableApps]);

  if (loading) {
    return <LoadingState label={i.workflows.loading} />;
  }

  if (loadError) {
    return (
      <ErrorState
        title={zh ? "工作流加载失败" : "Failed to load workflows"}
        message={loadError}
        retryLabel={i.common.refresh}
        onRetry={() => {
          void load();
        }}
      />
    );
  }

  // ── Editor View ──
  if (editingFile) {
    return (
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <div
          className="page-header"
          style={{ flexShrink: 0, borderBottom: "1px solid var(--border)", paddingBottom: 12, marginBottom: 0 }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <button className="btn btn-ghost btn-icon-sm" onClick={closeEditor}>
              <ArrowLeft size={16} />
            </button>
            <div>
              <h2 className="page-title" style={{ margin: 0 }}>
                {editingFile.name}
              </h2>
              <p style={{ fontSize: 11, color: "var(--text-muted)", margin: 0 }}>
                <span style={{ color: TOOL_COLORS[editingFile.tool_id] || "var(--text-secondary)", fontWeight: 600 }}>
                  {editingFile.tool_name}
                </span>
                {" · "}
                {editingFile.file_name}
              </p>
            </div>
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            {hasChanges && (
              <span style={{ fontSize: 11, color: "var(--warning)", fontWeight: 500 }}>{i.workflows.unsaved}</span>
            )}
            {saved && (
              <span style={{ fontSize: 11, color: "var(--success)", fontWeight: 500 }}>{i.workflows.saved}</span>
            )}
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => setContent(originalContent)}
              disabled={!hasChanges}
            >
              <RotateCcw size={14} /> {i.workflows.revert}
            </button>
            <button
              className="btn btn-primary btn-sm"
              onClick={() => void handleSave()}
              disabled={!hasChanges || saving}
            >
              <Save size={14} /> {saving ? "..." : i.workflows.save}
            </button>
          </div>
        </div>
        <div style={{ flex: 1, overflow: "auto", marginTop: 8 }}>
          {loadingContent ? (
            <LoadingState />
          ) : (
            <Suspense fallback={<LoadingState />}>
              <MarkdownEditor value={content} onChange={setContent} />
            </Suspense>
          )}
        </div>
      </div>
    );
  }

  // ── List View ──
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.workflows.title}</h2>
          <p className="page-subtitle">
            {zh ? `共 ${visibleFiles.length} 个工作流` : `${visibleFiles.length} workflows`}
          </p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="btn btn-secondary btn-sm"
            onClick={handleImport}
            disabled={hermesWorkflowUnsupported || workflowCapableApps.length === 0}
          >
            <Upload size={14} />
            {i.workflows.importWorkflow}
          </button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => setShowInstall(!showInstall)}
            disabled={hermesWorkflowUnsupported || workflowCapableApps.length === 0}
          >
            <Plus size={14} />
            {i.workflows.installTemplate}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => void load()}>
            <RefreshCw size={14} />
          </button>
        </div>
      </div>

      <div style={{ flex: 1, overflowY: "auto" }}>
        {hermesWorkflowUnsupported && (
          <div className="section-card" style={{ marginBottom: 16 }}>
            <h3 style={{ fontSize: 15, fontWeight: 700, marginBottom: 8 }}>
              {zh ? "Hermes Workflow 暂不支持" : "Hermes workflows are not supported yet"}
            </h3>
            <p style={{ fontSize: 12, color: "var(--text-muted)", margin: 0, lineHeight: 1.7 }}>
              {zh
                ? "Hermes 当前没有与本页完全对等的 workflow 机制，因此这里只保留显式禁用态，不提供安装、导入或编辑入口。"
                : "Hermes does not expose a workflow mechanism that matches this page yet, so this tab stays visible only as an explicit unsupported state."}
            </p>
          </div>
        )}

        {/* Install Template Panel (inline, top of page) */}
        {showInstall && !hermesWorkflowUnsupported && (
          <div className="section-card" style={{ marginBottom: 16 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
              <h3 style={{ fontSize: 16, fontWeight: 700 }}>{i.workflows.templateTitle}</h3>
              <button className="btn btn-ghost btn-icon-sm" onClick={() => setShowInstall(false)}>
                <X size={16} />
              </button>
            </div>
            <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>{i.workflows.templateTip}</p>

            {/* Tool Selector */}
            <div style={{ display: "flex", gap: 6, marginBottom: 14 }}>
              <span style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: "28px" }}>
                {i.workflows.selectTool}:
              </span>
              {visibleToolTabs
                .filter((tab) => tab.id !== "all" && tab.id !== "hermes")
                .map((tab) => (
                  <button
                    key={tab.id}
                    className={`btn btn-xs ${installTool === tab.id ? "btn-primary" : "btn-ghost"}`}
                    onClick={() => setInstallTool(tab.id)}
                  >
                    {zh ? tab.label_zh : tab.label_en}
                  </button>
                ))}
            </div>

            {/* Template Grid */}
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))", gap: 8 }}>
              {templates.map((tmpl) => {
                const isInstalled = installedIds.has(tmpl.id);
                const isInstalling = installing === tmpl.id;
                return (
                  <div
                    key={tmpl.id}
                    className={`card ${!isInstalled ? "card-interactive" : ""}`}
                    style={{ padding: "12px 14px", opacity: isInstalled ? 0.55 : 1 }}
                    onClick={() => !isInstalled && !isInstalling && void handleInstall(tmpl.id)}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 8 }}>
                      <div style={{ minWidth: 0 }}>
                        <div style={{ fontWeight: 600, fontSize: 13 }}>{zh ? tmpl.name_zh : tmpl.name_en}</div>
                        <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 3 }}>
                          {zh ? tmpl.description_zh : tmpl.description_en}
                        </div>
                      </div>
                      {isInstalling ? (
                        <div className="spinner" style={{ width: 14, height: 14, flexShrink: 0 }} />
                      ) : isInstalled ? (
                        <span style={{ fontSize: 10, color: "var(--success)", fontWeight: 600, flexShrink: 0 }}>✓</span>
                      ) : (
                        <Download size={14} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* Tool Tabs */}
        <div style={{ display: "flex", gap: 4, marginBottom: 12, flexWrap: "wrap" }}>
          {visibleToolTabs.map((tab) => (
            <button
              key={tab.id}
              className={`btn btn-xs ${activeTab === tab.id ? "btn-primary" : "btn-ghost"}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {zh ? tab.label_zh : tab.label_en}
              {tab.id !== "all" && (
                <span style={{ marginLeft: 4, opacity: 0.7 }}>
                  ({visibleFiles.filter((f) => f.tool_id === tab.id).length})
                </span>
              )}
            </button>
          ))}
        </div>

        {/* Workflow Cards */}
        {hermesWorkflowUnsupported ? (
          <EmptyState
            icon={<GitBranch size={28} style={{ color: "var(--text-muted)" }} />}
            title={zh ? "Hermes 暂无可管理 Workflow" : "Hermes workflows are unavailable"}
            description={
              zh
                ? "当前版本只支持 Hermes 的 Provider、MCP、Skills 和 ConfigFiles 管理。"
                : "This release only supports Hermes provider, MCP, skills, and config-file management."
            }
          />
        ) : filteredFiles.length === 0 ? (
          <EmptyState
            icon={<GitBranch size={28} style={{ color: "var(--text-muted)" }} />}
            title={i.workflows.noWorkflows}
            description={i.workflows.noWorkflowsTip}
            action={
              <button className="btn btn-primary btn-sm" type="button" onClick={() => setShowInstall(true)}>
                <Plus size={14} />
                {i.workflows.installTemplate}
              </button>
            }
          />
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }} className="stagger">
            {filteredFiles.map((file) => (
              <WorkflowCard
                key={file.path}
                file={file}
                toolColor={TOOL_COLORS[file.tool_id] || "var(--text-secondary)"}
                disabledLabel={i.workflows.disabled}
                editLabel={i.workflows.edit}
                deleteLabel={i.workflows.delete}
                toggleTitle={file.disabled ? i.workflows.enable : i.workflows.disable}
                toggling={togglingPath === file.path}
                formatSize={formatSize}
                onEdit={handleOpenEditor}
                onToggle={handleToggleWorkflow}
                onDelete={handleRequestDelete}
              />
            ))}
          </div>
        )}
      </div>

      {/* Delete Confirmation Dialog */}
      {confirmDelete && (
        <div
          style={{
            position: "fixed",
            inset: 0,
            background: "var(--bg-overlay)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1000,
          }}
          onClick={() => setConfirmDelete(null)}
        >
          <div
            className="card"
            style={{ padding: 24, maxWidth: 420, width: "90%" }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 style={{ fontSize: 16, fontWeight: 700, marginBottom: 12 }}>{i.workflows.confirmDelete}</h3>
            <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 8, lineHeight: 1.6 }}>
              {confirmDelete.tool_name} / {confirmDelete.name}
            </p>
            <div className="code-block" style={{ fontSize: 11, marginBottom: 20 }}>
              {confirmDelete.path}
            </div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-secondary btn-sm" onClick={() => setConfirmDelete(null)}>
                {i.workflows.close}
              </button>
              <button
                className="btn btn-sm"
                style={{ background: "var(--danger)", color: "#fff" }}
                onClick={() => void handleDelete(confirmDelete)}
              >
                <Trash2 size={14} />
                {i.workflows.delete}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
