import { useState, useEffect, lazy, Suspense } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Save, RotateCcw, Plus, Trash2, Pencil, ArrowLeft, Download, X, GitBranch, Upload } from "lucide-react";
import { t } from "../lib/i18n";
import { showToast } from "../components/Toast";

const MarkdownEditor = lazy(() => import("../components/MarkdownEditor"));

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
];

const TOOL_COLORS: Record<string, string> = {
  claude: "#d97706",
  codex: "#2563eb",
  gemini: "#059669",
  opencode: "#7c3aed",
  openclaw: "#dc2626",
};

export default function Workflows() {
  const [files, setFiles] = useState<WorkflowFile[]>([]);
  const [templates, setTemplates] = useState<WorkflowTemplate[]>([]);
  const [loading, setLoading] = useState(true);
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
  const i = t();
  const zh = (localStorage.getItem("cchub-locale") || "zh") === "zh";

  useEffect(() => { load(); }, []);

  async function load() {
    setLoading(true);
    try {
      const [f, tmpl] = await Promise.all([
        invoke<WorkflowFile[]>("scan_workflows"),
        invoke<WorkflowTemplate[]>("get_workflow_templates"),
      ]);
      setFiles(f);
      setTemplates(tmpl);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }

  async function openEditor(file: WorkflowFile) {
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
    }
    finally { setLoadingContent(false); }
  }

  function closeEditor() {
    setEditingFile(null);
    setContent("");
    setOriginalContent("");
  }

  async function handleSave() {
    if (!editingFile) return;
    setSaving(true);
    try {
      await invoke("write_workflow_content", { path: editingFile.path, content });
      setOriginalContent(content);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      showToast("success", zh ? "已保存" : "Saved");
    } catch (e) { showToast("error", `${e}`); }
    finally { setSaving(false); }
  }

  async function handleDelete(file: WorkflowFile) {
    try {
      await invoke("delete_workflow", { path: file.path });
      showToast("success", zh ? "已删除" : "Deleted");
      if (editingFile?.path === file.path) closeEditor();
      setConfirmDelete(null);
      await load();
    } catch (e) { showToast("error", `${e}`); }
  }

  async function handleToggle(file: WorkflowFile) {
    setTogglingPath(file.path);
    try {
      await invoke<string>("toggle_workflow", { path: file.path, enabled: file.disabled });
      showToast("success", zh ? "已更新" : "Updated");
      await load();
    } catch (e) { showToast("error", `${e}`); }
    finally { setTogglingPath(null); }
  }

  async function handleInstall(templateId: string) {
    setInstalling(templateId);
    try {
      await invoke<string>("install_workflow", { toolId: installTool, templateId });
      showToast("success", zh ? "安装成功" : "Installed");
      await load();
    } catch (e) { showToast("error", `${e}`); }
    finally { setInstalling(null); }
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  const filteredFiles = activeTab === "all" ? files : files.filter(f => f.tool_id === activeTab);
  const hasChanges = content !== originalContent;

  async function handleImport() {
    try {
      await invoke<string>("import_workflow_file", { toolId: activeTab === "all" ? "claude" : activeTab });
      showToast("success", i.workflows.importSuccess);
      await load();
    } catch (e) {
      const msg = String(e);
      if (msg !== "Cancelled") showToast("error", msg);
    }
  }

  const installedIds = new Set(
    files.filter(f => f.tool_id === installTool).map(f => f.file_name.replace(".md", "").replace(".disabled", ""))
  );

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>{i.workflows.loading}</span>
      </div>
    );
  }

  // ── Editor View ──
  if (editingFile) {
    return (
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <div className="page-header" style={{ flexShrink: 0, borderBottom: "1px solid var(--border)", paddingBottom: 12, marginBottom: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <button className="btn btn-ghost btn-icon-sm" onClick={closeEditor}><ArrowLeft size={16} /></button>
            <div>
              <h2 className="page-title" style={{ margin: 0 }}>{editingFile.name}</h2>
              <p style={{ fontSize: 11, color: "var(--text-muted)", margin: 0 }}>
                <span style={{ color: TOOL_COLORS[editingFile.tool_id] || "var(--text-secondary)", fontWeight: 600 }}>
                  {editingFile.tool_name}
                </span>
                {" · "}{editingFile.file_name}
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
            <button className="btn btn-secondary btn-sm" onClick={() => setContent(originalContent)} disabled={!hasChanges}>
              <RotateCcw size={14} /> {i.workflows.revert}
            </button>
            <button className="btn btn-primary btn-sm" onClick={() => void handleSave()} disabled={!hasChanges || saving}>
              <Save size={14} /> {saving ? "..." : i.workflows.save}
            </button>
          </div>
        </div>
        <div style={{ flex: 1, overflow: "auto", marginTop: 8 }}>
          {loadingContent ? (
            <div className="loading-center"><div className="spinner" /></div>
          ) : (
            <Suspense fallback={<div className="loading-center"><div className="spinner" /></div>}>
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
          <p className="page-subtitle">{zh ? `共 ${files.length} 个工作流` : `${files.length} workflows`}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={handleImport}>
            <Upload size={14} />{i.workflows.importWorkflow}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => setShowInstall(!showInstall)}>
            <Plus size={14} />{i.workflows.installTemplate}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => void load()}>
            <RefreshCw size={14} />
          </button>
        </div>
      </div>

      <div style={{ flex: 1, overflowY: "auto" }}>
        {/* Install Template Panel (inline, top of page) */}
        {showInstall && (
          <div className="section-card" style={{ marginBottom: 16 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
              <h3 style={{ fontSize: 16, fontWeight: 700 }}>{i.workflows.templateTitle}</h3>
              <button className="btn btn-ghost btn-icon-sm" onClick={() => setShowInstall(false)}><X size={16} /></button>
            </div>
            <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>{i.workflows.templateTip}</p>

            {/* Tool Selector */}
            <div style={{ display: "flex", gap: 6, marginBottom: 14 }}>
              <span style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: "28px" }}>{i.workflows.selectTool}:</span>
              {TOOL_TABS.filter(tab => tab.id !== "all").map(tab => (
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
              {templates.map(tmpl => {
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
          {TOOL_TABS.map(tab => (
            <button
              key={tab.id}
              className={`btn btn-xs ${activeTab === tab.id ? "btn-primary" : "btn-ghost"}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {zh ? tab.label_zh : tab.label_en}
              {tab.id !== "all" && (
                <span style={{ marginLeft: 4, opacity: 0.7 }}>
                  ({files.filter(f => f.tool_id === tab.id).length})
                </span>
              )}
            </button>
          ))}
        </div>

        {/* Workflow Cards */}
        {filteredFiles.length === 0 ? (
          <div className="card empty-state" style={{ flex: 1 }}>
            <div className="empty-icon"><GitBranch size={28} style={{ color: "var(--text-muted)" }} /></div>
            <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{i.workflows.noWorkflows}</p>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8, maxWidth: 320 }}>{i.workflows.noWorkflowsTip}</p>
            <button className="btn btn-primary btn-sm" style={{ marginTop: 16 }} onClick={() => setShowInstall(true)}>
              <Plus size={14} />{i.workflows.installTemplate}
            </button>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }} className="stagger">
            {filteredFiles.map(file => (
              <div
                key={file.path}
                className="card"
                style={{ padding: "16px 20px", opacity: file.disabled ? 0.55 : 1, transition: "opacity 0.2s" }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                  <div className="icon-box" style={{ background: "var(--bg-elevated)", width: 36, height: 36, borderRadius: 6 }}>
                    <GitBranch size={16} style={{ color: file.disabled ? "var(--text-muted)" : TOOL_COLORS[file.tool_id] || "var(--text-secondary)" }} />
                  </div>
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      <span style={{ fontSize: 13, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {file.name}
                      </span>
                      <span style={{
                        fontSize: 10, padding: "1px 6px", borderRadius: 4, fontWeight: 600,
                        background: `${TOOL_COLORS[file.tool_id] || "#666"}15`,
                        color: TOOL_COLORS[file.tool_id] || "#666",
                      }}>{file.tool_name}</span>
                      {file.disabled && (
                        <span className="badge badge-muted" style={{ fontSize: 10, padding: "1px 6px" }}>
                          {i.workflows.disabled}
                        </span>
                      )}
                      <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{formatSize(file.size_bytes)}</span>
                      {file.modified_at && (
                        <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{file.modified_at}</span>
                      )}
                    </div>
                    <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 3, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {file.path}
                    </div>
                  </div>

                  {/* Actions — matches ClaudeMd pattern */}
                  <div style={{ display: "flex", alignItems: "center", gap: 10, flexShrink: 0 }}>
                    <button className="btn btn-secondary btn-sm" onClick={() => void openEditor(file)}>
                      <Pencil size={13} />{i.workflows.edit}
                    </button>

                    {/* Toggle switch */}
                    <button
                      onClick={() => void handleToggle(file)}
                      disabled={togglingPath === file.path}
                      title={file.disabled ? i.workflows.enable : i.workflows.disable}
                      style={{
                        position: "relative", width: 40, height: 22, borderRadius: 11,
                        border: "none", padding: 0, flexShrink: 0,
                        cursor: togglingPath === file.path ? "wait" : "pointer",
                        background: file.disabled ? "var(--border-strong)" : "var(--success)",
                        transition: "background 0.2s",
                      }}
                    >
                      <span style={{
                        position: "absolute", top: 2,
                        left: file.disabled ? 2 : 20,
                        width: 18, height: 18, borderRadius: "50%",
                        background: "#fff", transition: "left 0.2s",
                        boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
                      }} />
                    </button>

                    <button
                      className="btn btn-ghost btn-icon-sm"
                      title={i.workflows.delete}
                      onClick={() => setConfirmDelete(file)}
                    >
                      <Trash2 size={14} style={{ color: "var(--danger)" }} />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Delete Confirmation Dialog */}
      {confirmDelete && (
        <div
          style={{
            position: "fixed", inset: 0, background: "var(--bg-overlay)",
            display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000,
          }}
          onClick={() => setConfirmDelete(null)}
        >
          <div
            className="card"
            style={{ padding: 24, maxWidth: 420, width: "90%" }}
            onClick={e => e.stopPropagation()}
          >
            <h3 style={{ fontSize: 16, fontWeight: 700, marginBottom: 12 }}>{i.workflows.confirmDelete}</h3>
            <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 8, lineHeight: 1.6 }}>
              {confirmDelete.tool_name} / {confirmDelete.name}
            </p>
            <div className="code-block" style={{ fontSize: 11, marginBottom: 20 }}>{confirmDelete.path}</div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-secondary btn-sm" onClick={() => setConfirmDelete(null)}>
                {i.workflows.close}
              </button>
              <button
                className="btn btn-sm"
                style={{ background: "var(--danger)", color: "#fff" }}
                onClick={() => void handleDelete(confirmDelete)}
              >
                <Trash2 size={14} />{i.workflows.delete}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
