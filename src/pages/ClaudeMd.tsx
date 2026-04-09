import { useState, useEffect, lazy, Suspense, useMemo, useCallback, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, FileText, Save, RotateCcw, Plus, X, Check, Trash2, Pencil, ArrowLeft, Search } from "lucide-react";
import { t } from "../lib/i18n";
import ClaudeMdFileCard from "../components/ClaudeMdFileCard";
import ClaudeMdPresetCard from "../components/ClaudeMdPresetCard";
import ClaudeMdTemplateCard from "../components/ClaudeMdTemplateCard";
import { showToast } from "../components/Toast";
import { fetchVisibleApps, toolNameToAppId } from "../lib/appPreferences";

const MarkdownEditor = lazy(() => import("../components/MarkdownEditor"));
const CodeEditor = lazy(() => import("../components/CodeEditor"));

interface ClaudeMdFile {
  path: string;
  project_name: string;
  size_bytes: number;
  modified_at: string | null;
  content_preview: string;
  disabled: boolean;
  tool_name: string;
  file_name: string;
  scope: string;
}

interface ClaudeMdTemplate {
  id: string;
  name: string;
  description: string;
  content: string;
  file_name: string;
  tool_name: string;
}

interface PromptPreset {
  id: string;
  name: string;
  content: string;
  created_at: string;
  updated_at: string;
}

interface PromptPresetState {
  presets: PromptPreset[];
  active_preset_id: string | null;
}

export default function ClaudeMd() {
  const [files, setFiles] = useState<ClaudeMdFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [editingFile, setEditingFile] = useState<ClaudeMdFile | null>(null);
  const [content, setContent] = useState("");
  const [originalContent, setOriginalContent] = useState("");
  const [loadingContent, setLoadingContent] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [templates, setTemplates] = useState<ClaudeMdTemplate[]>([]);
  const [showCreate, setShowCreate] = useState(false);
  const [newDirPath, setNewDirPath] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<ClaudeMdFile | null>(null);
  const [togglingPath, setTogglingPath] = useState<string | null>(null);
  const [promptPresets, setPromptPresets] = useState<PromptPreset[]>([]);
  const [activePresetId, setActivePresetId] = useState<string | null>(null);
  const [showPresetEditor, setShowPresetEditor] = useState(false);
  const [editingPresetId, setEditingPresetId] = useState<string | null>(null);
  const [presetName, setPresetName] = useState("");
  const [presetContent, setPresetContent] = useState("");
  const [savingPreset, setSavingPreset] = useState(false);
  const [activatingPresetId, setActivatingPresetId] = useState<string | null>(null);
  const [confirmPresetDelete, setConfirmPresetDelete] = useState<PromptPreset | null>(null);
  const [search, setSearch] = useState("");
  const i = t();
  const locale = localStorage.getItem("cchub-locale") || "zh";
  const hasChanges = content !== originalContent;

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [f, tmpl, nextVisibleApps, presetState] = await Promise.all([
        invoke<ClaudeMdFile[]>("scan_claude_md"),
        invoke<ClaudeMdTemplate[]>("get_claude_md_templates"),
        fetchVisibleApps(),
        invoke<PromptPresetState>("get_prompt_presets").catch(() => ({ presets: [], active_preset_id: null })),
      ]);
      setFiles(f.filter((file) => {
        const appId = toolNameToAppId(file.tool_name);
        return appId ? nextVisibleApps.includes(appId) : true;
      }));
      setTemplates(tmpl.filter((template) => {
        const appId = toolNameToAppId(template.tool_name);
        return appId ? nextVisibleApps.includes(appId) : true;
      }));
      setPromptPresets(presetState.presets);
      setActivePresetId(presetState.active_preset_id);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }, []);

  const openPresetEditor = useCallback((preset?: PromptPreset) => {
    setShowPresetEditor(true);
    setEditingPresetId(preset?.id || null);
    setPresetName(preset?.name || "");
    setPresetContent(preset?.content || "");
  }, []);

  const closePresetEditor = useCallback(() => {
    setShowPresetEditor(false);
    setEditingPresetId(null);
    setPresetName("");
    setPresetContent("");
  }, []);

  const openEditor = useCallback(async (file: ClaudeMdFile) => {
    setEditingFile(file);
    setLoadingContent(true);
    try {
      const c = await invoke<string>("read_claude_md_content", { path: file.path });
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

  const handleSave = useCallback(async () => {
    if (!editingFile) return;
    setSaving(true);
    try {
      await invoke("write_claude_md_content", { path: editingFile.path, content });
      setOriginalContent(content);
      setSaved(true);
      window.setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error(e);
      showToast("error", "Failed to save");
    } finally {
      setSaving(false);
    }
  }, [content, editingFile]);

  const handleRevert = useCallback(() => {
    setContent(originalContent);
  }, [originalContent]);

  const handleRequestCreate = useCallback(() => {
    setShowCreate(true);
  }, []);

  const handleCloseCreate = useCallback(() => {
    setShowCreate(false);
  }, []);

  const handleRequestDelete = useCallback((file: ClaudeMdFile) => {
    setConfirmDelete(file);
  }, []);

  const handleRequestPresetDelete = useCallback((preset: PromptPreset) => {
    setConfirmPresetDelete(preset);
  }, []);

  const handleClearSearch = useCallback(() => {
    setSearch("");
  }, []);

  const handleRefresh = useCallback(() => {
    void load();
  }, [load]);

  const handlePresetNameChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setPresetName(event.target.value);
  }, []);

  const handlePresetContentChange = useCallback((value: string) => {
    setPresetContent(value);
  }, []);

  const handleSearchChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setSearch(event.target.value);
  }, []);

  const handleNewDirPathChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setNewDirPath(event.target.value);
  }, []);

  const savePreset = useCallback(async () => {
    setSavingPreset(true);
    try {
      await invoke("save_prompt_preset", { id: editingPresetId, name: presetName, content: presetContent });
      await load();
      closePresetEditor();
      showToast("success", locale === "zh" ? "预设已保存" : "Preset saved");
    } catch (e: any) {
      showToast("error", e?.toString() || "Failed to save preset");
    } finally {
      setSavingPreset(false);
    }
  }, [closePresetEditor, editingPresetId, load, locale, presetContent, presetName]);

  const handleCreate = useCallback(async (template: ClaudeMdTemplate) => {
    if (!newDirPath.trim()) return;
    try {
      const dirPath = newDirPath.trim();
      const path = await invoke<string>("create_instruction_doc_file", {
        dirPath,
        fileName: template.file_name,
        content: template.content,
      });
      setShowCreate(false);
      setNewDirPath("");
      await load();
      const newFile: ClaudeMdFile = {
        path,
        project_name: dirPath.split(/[/\\]/).pop() || dirPath,
        size_bytes: template.content.length,
        modified_at: new Date().toISOString().slice(0, 16).replace("T", " "),
        content_preview: template.content.slice(0, 200),
        disabled: false,
        tool_name: template.tool_name,
        file_name: template.file_name,
        scope: "project",
      };
      void openEditor(newFile);
    } catch (e: any) {
      showToast("error", e?.toString() || "Failed to create file");
    }
  }, [load, newDirPath, openEditor]);

  const handleDelete = useCallback(async (file: ClaudeMdFile) => {
    try {
      await invoke("delete_claude_md_file", { path: file.path });
      showToast("success", i.claudeMd.deleteSuccess);
      if (editingFile?.path === file.path) {
        closeEditor();
      }
      setConfirmDelete(null);
      await load();
    } catch (e: any) {
      showToast("error", e?.toString() || "Failed to delete");
    }
  }, [closeEditor, editingFile?.path, i.claudeMd.deleteSuccess, load]);

  const handleToggle = useCallback(async (file: ClaudeMdFile) => {
    setTogglingPath(file.path);
    try {
      if (file.disabled) {
        await invoke<string>("enable_claude_md_file", { path: file.path });
        showToast("success", i.claudeMd.enableSuccess);
      } else {
        await invoke<string>("disable_claude_md_file", { path: file.path });
        showToast("success", i.claudeMd.disableSuccess);
      }
      await load();
    } catch (e: any) {
      showToast("error", e?.toString() || "Failed to toggle");
    } finally {
      setTogglingPath(null);
    }
  }, [i.claudeMd.disableSuccess, i.claudeMd.enableSuccess, load]);

  const handleActivatePreset = useCallback(async (presetId: string) => {
    setActivatingPresetId(presetId);
    try {
      await invoke("activate_prompt_preset", { id: presetId });
      await load();
      showToast("success", locale === "zh" ? "预设已激活并同步到全局文档" : "Preset activated and synced to global docs");
    } catch (e: any) {
      showToast("error", e?.toString() || "Failed to activate preset");
    } finally {
      setActivatingPresetId((current) => current === presetId ? null : current);
    }
  }, [load, locale]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    const handleEscape = () => {
      if (confirmDelete) {
        setConfirmDelete(null);
        return;
      }
      if (confirmPresetDelete) {
        setConfirmPresetDelete(null);
        return;
      }
      if (showCreate) {
        setShowCreate(false);
        return;
      }
      if (showPresetEditor) {
        setShowPresetEditor(false);
        setEditingPresetId(null);
        return;
      }
      if (editingFile) {
        closeEditor();
      }
    };
    window.addEventListener("cchub-shortcut-escape", handleEscape);
    return () => window.removeEventListener("cchub-shortcut-escape", handleEscape);
  }, [confirmDelete, confirmPresetDelete, editingFile, showCreate, showPresetEditor]);
  useEffect(() => {
    const handleSaveShortcut = () => {
      if (editingFile && hasChanges && !saving) {
        void handleSave();
      }
    };
    const handleNewShortcut = () => {
      if (!editingFile && !showPresetEditor) {
        setShowCreate(true);
      }
    };
    window.addEventListener("cchub-shortcut-save", handleSaveShortcut);
    window.addEventListener("cchub-shortcut-new", handleNewShortcut);
    return () => {
      window.removeEventListener("cchub-shortcut-save", handleSaveShortcut);
      window.removeEventListener("cchub-shortcut-new", handleNewShortcut);
    };
  }, [editingFile, handleSave, hasChanges, saving, showPresetEditor]);

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  function isMarkdownFile(path: string): boolean {
    return path.endsWith(".md") || path.endsWith(".md.bak");
  }

  const filteredFiles = useMemo(() => {
    const keyword = search.trim().toLowerCase();
    if (!keyword) return files;
    return files.filter((file) => (
      file.project_name.toLowerCase().includes(keyword)
      || file.tool_name.toLowerCase().includes(keyword)
      || file.file_name.toLowerCase().includes(keyword)
      || file.path.toLowerCase().includes(keyword)
      || file.content_preview.toLowerCase().includes(keyword)
    ));
  }, [files, search]);

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
          {locale === "zh" ? "正在扫描配置文件..." : "Scanning config files..."}
        </span>
      </div>
    );
  }

  // ── Editor Page View ──
  if (editingFile) {
    return (
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        {/* Header */}
        <div className="page-header">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <button className="btn btn-ghost btn-icon-sm" onClick={closeEditor}>
              <ArrowLeft size={16} />
            </button>
            <FileText size={18} style={{ color: "var(--text-secondary)" }} />
            <h2 className="page-title" style={{ margin: 0 }}>{editingFile.project_name}</h2>
            {editingFile.disabled && (
              <span className="badge badge-muted" style={{ fontSize: 10 }}>{i.claudeMd.disabled}</span>
            )}
            <span className="badge badge-accent">{formatSize(editingFile.size_bytes)}</span>
            {hasChanges && (
              <span className="badge badge-warning">{locale === "zh" ? "未保存" : "Unsaved"}</span>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            {hasChanges && (
              <button className="btn btn-secondary btn-sm" onClick={handleRevert}>
                <RotateCcw size={14} />{i.claudeMd.revert}
              </button>
            )}
            <button
              className="btn btn-primary btn-sm"
              onClick={handleSave}
              disabled={!hasChanges || saving}
            >
              {saved ? <Check size={14} /> : <Save size={14} />}
              {saved ? i.claudeMd.saved : i.common.save}
            </button>
          </div>
        </div>

        {/* File Path */}
        <div style={{ marginBottom: 16 }}>
          <div className="code-block" style={{ fontSize: 11 }}>{editingFile.path}</div>
        </div>

        {/* Editor */}
        <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
          {loadingContent ? (
            <div style={{ display: "flex", alignItems: "center", gap: 10, padding: 40, justifyContent: "center" }}>
              <div className="spinner" style={{ width: 18, height: 18 }} />
              <span style={{ fontSize: 12, color: "var(--text-muted)" }}>Loading...</span>
            </div>
          ) : isMarkdownFile(editingFile.path) ? (
            <Suspense fallback={
              <div style={{ display: "flex", alignItems: "center", gap: 10, padding: 40, justifyContent: "center" }}>
                <div className="spinner" style={{ width: 18, height: 18 }} />
              </div>
            }>
              <MarkdownEditor
                value={content}
                onChange={setContent}
                minHeight={500}
              />
            </Suspense>
          ) : (
            <CodeEditor
              value={content}
              onChange={setContent}
              language="json"
              minHeight={500}
            />
          )}
        </div>
      </div>
    );
  }

  // ── File List View ──
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.claudeMd.title}</h2>
          <p className="page-subtitle">{i.claudeMd.subtitle.replace("{count}", String(files.length))}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={handleRequestCreate}>
            <Plus size={14} />{i.claudeMd.newFile}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={handleRefresh}>
            <RefreshCw size={14} />{i.common.refresh}
          </button>
        </div>
      </div>

      <div className="section-card" style={{ marginBottom: 16 }}>
        <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start", marginBottom: 14, flexWrap: "wrap" }}>
          <div>
            <div className="section-card-title" style={{ marginBottom: 4 }}>
              <Pencil size={16} style={{ color: "var(--text-secondary)" }} />
              {locale === "zh" ? "Prompt 预设" : "Prompt Presets"}
            </div>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {locale === "zh"
                ? "创建多套通用指令预设，并一键同步写入 Claude / Codex / Gemini / OpenCode / OpenClaw 的全局指令文档。"
              : "Create reusable instruction presets and sync them into the global docs for Claude, Codex, Gemini, OpenCode, and OpenClaw."}
            </p>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={() => openPresetEditor()} style={{ gap: 6 }}>
            <Plus size={14} />{locale === "zh" ? "新建预设" : "New Preset"}
          </button>
        </div>

        {showPresetEditor && (
          <div style={{ marginBottom: 14, padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
            <div style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "center", marginBottom: 10 }}>
              <div style={{ fontSize: 13, fontWeight: 600 }}>
                {editingPresetId ? (locale === "zh" ? "编辑预设" : "Edit Preset") : (locale === "zh" ? "新建预设" : "Create Preset")}
              </div>
              <button className="btn btn-ghost btn-icon-sm" onClick={closePresetEditor}>
                <X size={14} />
              </button>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <input
                className="input"
                placeholder={locale === "zh" ? "预设名称" : "Preset name"}
                value={presetName}
                onChange={handlePresetNameChange}
              />
              <CodeEditor
                value={presetContent}
                onChange={handlePresetContentChange}
                language="markdown"
                minHeight={220}
              />
              <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
                <button className="btn btn-secondary btn-sm" onClick={closePresetEditor}>
                  <X size={14} />{locale === "zh" ? "取消" : "Cancel"}
                </button>
                <button className="btn btn-primary btn-sm" onClick={savePreset} disabled={savingPreset || !presetName.trim()}>
                  <Save size={14} />{savingPreset ? (locale === "zh" ? "保存中..." : "Saving...") : (locale === "zh" ? "保存预设" : "Save Preset")}
                </button>
              </div>
            </div>
          </div>
        )}

        {promptPresets.length === 0 ? (
          <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
            {locale === "zh" ? "暂无预设。创建后可一键激活并同步到多个工具。" : "No presets yet. Create one and activate it across multiple tools."}
          </div>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 10 }}>
            {promptPresets.map((preset) => (
              <ClaudeMdPresetCard
                key={preset.id}
                preset={preset}
                isActive={activePresetId === preset.id}
                activating={activatingPresetId === preset.id}
                activeLabel={locale === "zh" ? "当前激活" : "Active"}
                activationHint={locale === "zh" ? "激活后将写入所有全局指令文档" : "Activation writes all global instruction docs"}
                editTitle={locale === "zh" ? "编辑预设" : "Edit preset"}
                deleteTitle={locale === "zh" ? "删除预设" : "Delete preset"}
                syncLabel={locale === "zh" ? "激活" : "Activate"}
                resyncLabel={locale === "zh" ? "重新同步" : "Resync"}
                syncingLabel={locale === "zh" ? "同步中..." : "Syncing..."}
                onEdit={openPresetEditor}
                onDelete={handleRequestPresetDelete}
                onActivate={handleActivatePreset}
              />
            ))}
          </div>
        )}
      </div>

      {files.length === 0 && !showCreate ? (
        <div className="card empty-state" style={{ flex: 1 }}>
          <div className="empty-icon"><FileText size={28} style={{ color: "var(--text-muted)" }} /></div>
          <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{i.claudeMd.noFiles}</p>
          <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8, maxWidth: 320 }}>{i.claudeMd.noFilesTip}</p>
          <button className="btn btn-primary btn-sm" style={{ marginTop: 16 }} onClick={handleRequestCreate}>
            <Plus size={14} />{i.claudeMd.newFile}
          </button>
        </div>
      ) : (
        <div style={{ flex: 1, overflowY: "auto" }}>
          <div style={{ position: "relative", marginBottom: 16 }}>
            <Search size={14} style={{ position: "absolute", left: 12, top: "50%", transform: "translateY(-50%)", color: "var(--text-muted)" }} />
            <input
              className="input"
              style={{ paddingLeft: 36, paddingRight: search ? 36 : undefined }}
              placeholder={locale === "zh" ? "搜索指令文档、路径或预览内容..." : "Search docs, paths, or preview content..."}
              value={search}
              onChange={handleSearchChange}
            />
            {search && (
              <button
                className="btn btn-ghost btn-icon-sm"
                style={{ position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)" }}
                onClick={handleClearSearch}
              >
                <X size={14} />
              </button>
            )}
          </div>

          {/* New File Panel */}
          {showCreate && (
            <div className="section-card" style={{ marginBottom: 16 }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}>
                <h3 style={{ fontSize: 16, fontWeight: 700 }}>{i.claudeMd.newFile}</h3>
                <button className="btn btn-ghost btn-icon-sm" onClick={handleCloseCreate}><X size={16} /></button>
              </div>
              <div style={{ marginBottom: 16 }}>
                <span className="field-label">{i.claudeMd.createIn}</span>
                <input
                  className="input"
                  style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 13 }}
                  placeholder={locale === "zh" ? "输入项目目录路径" : "Enter project directory path"}
                  value={newDirPath}
                  onChange={handleNewDirPathChange}
                />
              </div>
              <span className="field-label">{i.claudeMd.selectTemplate}</span>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))", gap: 8 }}>
                {templates.map((tmpl) => (
                  <ClaudeMdTemplateCard
                    key={tmpl.id}
                    template={tmpl}
                    onCreate={handleCreate}
                  />
                ))}
              </div>
            </div>
          )}

          {/* File List */}
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }} className="stagger">
            {filteredFiles.length === 0 ? (
              <div className="card" style={{ padding: "24px 20px", textAlign: "center" }}>
                <p style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)" }}>
                  {locale === "zh" ? "没有匹配的指令文档" : "No matching instruction docs"}
                </p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 6 }}>
                  {locale === "zh" ? "尝试修改关键字，或清空搜索查看全部文件。" : "Try another keyword or clear the search to view all files."}
                </p>
              </div>
            ) : filteredFiles.map((file) => (
              <ClaudeMdFileCard
                key={file.path}
                file={file}
                disabledLabel={i.claudeMd.disabled}
                editLabel={i.common.edit}
                deleteTitle={i.claudeMd.delete}
                enableTitle={i.claudeMd.enable}
                disableTitle={i.claudeMd.disable}
                metaLabel={`${file.tool_name} · ${file.file_name} · ${file.scope === "project" ? "Project" : "Global"}`}
                sizeLabel={formatSize(file.size_bytes)}
                toggling={togglingPath === file.path}
                onEdit={openEditor}
                onToggle={handleToggle}
                onDelete={handleRequestDelete}
              />
            ))}
          </div>
        </div>
      )}

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
            onClick={(e) => e.stopPropagation()}
          >
            <h3 style={{ fontSize: 16, fontWeight: 700, marginBottom: 12 }}>{i.claudeMd.delete}</h3>
            <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 20, lineHeight: 1.6 }}>
              {i.claudeMd.deleteConfirm.replace("{name}", confirmDelete.project_name)}
            </p>
            <div className="code-block" style={{ fontSize: 11, marginBottom: 20 }}>{confirmDelete.path}</div>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-secondary btn-sm" onClick={() => setConfirmDelete(null)}>
                {i.common.cancel}
              </button>
              <button
                className="btn btn-sm"
                style={{ background: "var(--danger)", color: "#fff" }}
                onClick={() => handleDelete(confirmDelete)}
              >
                <Trash2 size={14} />{i.claudeMd.delete}
              </button>
            </div>
          </div>
        </div>
      )}

      {confirmPresetDelete && (
        <div
          style={{
            position: "fixed", inset: 0, background: "var(--bg-overlay)",
            display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000,
          }}
          onClick={() => setConfirmPresetDelete(null)}
        >
          <div
            className="card"
            style={{ padding: 24, maxWidth: 460, width: "90%" }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 style={{ fontSize: 16, fontWeight: 700, marginBottom: 12 }}>{locale === "zh" ? "删除预设" : "Delete Preset"}</h3>
            <p style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 20, lineHeight: 1.6 }}>
              {locale === "zh"
                ? `确定删除「${confirmPresetDelete.name}」？如果它当前处于激活状态，只会删除预设记录，不会删除已写入的全局文档。`
                : `Delete "${confirmPresetDelete.name}"? If it is currently active, only the preset record will be removed and the written global docs will remain.`}
            </p>
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
              <button className="btn btn-secondary btn-sm" onClick={() => setConfirmPresetDelete(null)}>
                {i.common.cancel}
              </button>
              <button
                className="btn btn-sm"
                style={{ background: "var(--danger)", color: "#fff" }}
                onClick={async () => {
                  const preset = confirmPresetDelete;
                  if (!preset) return;
                  try {
                    await invoke("delete_prompt_preset", { id: preset.id });
                    setConfirmPresetDelete(null);
                    await load();
                    showToast("success", locale === "zh" ? "预设已删除" : "Preset deleted");
                  } catch (e: any) {
                    showToast("error", e?.toString() || "Failed to delete preset");
                  }
                }}
              >
                <Trash2 size={14} />{locale === "zh" ? "删除预设" : "Delete Preset"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
