import { useState, useEffect, lazy, Suspense, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, FileText, Save, RotateCcw, Plus, X, Check, Trash2, Pencil, ArrowLeft, Search } from "lucide-react";
import { t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import CodeEditor from "../components/CodeEditor";
import { fetchVisibleApps, toolNameToAppId } from "../lib/appPreferences";

const MarkdownEditor = lazy(() => import("../components/MarkdownEditor"));

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
  const hasChanges = content !== originalContent;

  useEffect(() => { load(); }, []);
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
  }, [editingFile, hasChanges, saving, showPresetEditor]);

  async function load() {
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
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  }

  function openPresetEditor(preset?: PromptPreset) {
    setShowPresetEditor(true);
    setEditingPresetId(preset?.id || null);
    setPresetName(preset?.name || "");
    setPresetContent(preset?.content || "");
  }

  function closePresetEditor() {
    setShowPresetEditor(false);
    setEditingPresetId(null);
    setPresetName("");
    setPresetContent("");
  }

  async function savePreset() {
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
  }

  async function openEditor(file: ClaudeMdFile) {
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
      await invoke("write_claude_md_content", { path: editingFile.path, content });
      setOriginalContent(content);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) { console.error(e); showToast("error", "Failed to save"); }
    finally { setSaving(false); }
  }

  function handleRevert() {
    setContent(originalContent);
  }

  async function handleCreate(template: ClaudeMdTemplate) {
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
      openEditor(newFile);
    } catch (e: any) {
      showToast("error", e?.toString() || "Failed to create file");
    }
  }

  async function handleDelete(file: ClaudeMdFile) {
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
  }

  async function handleToggle(file: ClaudeMdFile) {
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
    }
    finally { setTogglingPath(null); }
  }

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

  const locale = localStorage.getItem("cchub-locale") || "zh";

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
          <button className="btn btn-secondary btn-sm" onClick={() => setShowCreate(true)}>
            <Plus size={14} />{i.claudeMd.newFile}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={load}>
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
                onChange={(e) => setPresetName(e.target.value)}
              />
              <CodeEditor
                value={presetContent}
                onChange={setPresetContent}
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
            {promptPresets.map((preset) => {
              const isActive = activePresetId === preset.id;
              return (
                <div key={preset.id} className="card" style={{ padding: "14px 16px", background: isActive ? "var(--bg-elevated)" : "var(--bg-card)" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "flex-start", marginBottom: 10 }}>
                    <div style={{ minWidth: 0 }}>
                      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                        <span style={{ fontSize: 14, fontWeight: 700 }}>{preset.name}</span>
                        {isActive && <span className="badge badge-success" style={{ fontSize: 10 }}>{locale === "zh" ? "当前激活" : "Active"}</span>}
                      </div>
                      <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
                        {preset.updated_at.replace("T", " ").slice(0, 19)}
                      </div>
                    </div>
                    <div style={{ display: "flex", gap: 4, flexShrink: 0 }}>
                      <button className="btn btn-ghost btn-icon-sm" onClick={() => openPresetEditor(preset)} title={locale === "zh" ? "编辑预设" : "Edit preset"}>
                        <Pencil size={14} />
                      </button>
                      <button className="btn btn-ghost btn-icon-sm" onClick={() => setConfirmPresetDelete(preset)} title={locale === "zh" ? "删除预设" : "Delete preset"}>
                        <Trash2 size={14} style={{ color: "var(--danger)" }} />
                      </button>
                    </div>
                  </div>
                  <div className="code-block" style={{ fontSize: 11, maxHeight: 120, overflow: "auto", marginBottom: 10 }}>
                    {preset.content}
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                    <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                      {locale === "zh" ? "激活后将写入所有全局指令文档" : "Activation writes all global instruction docs"}
                    </div>
                    <button
                      className={`btn btn-sm ${isActive ? "btn-secondary" : "btn-primary"}`}
                      disabled={activatingPresetId === preset.id}
                      onClick={async () => {
                        setActivatingPresetId(preset.id);
                        try {
                          await invoke("activate_prompt_preset", { id: preset.id });
                          await load();
                          showToast("success", locale === "zh" ? "预设已激活并同步到全局文档" : "Preset activated and synced to global docs");
                        } catch (e: any) {
                          showToast("error", e?.toString() || "Failed to activate preset");
                        } finally {
                          setActivatingPresetId((current) => current === preset.id ? null : current);
                        }
                      }}
                    >
                      {activatingPresetId === preset.id
                        ? (locale === "zh" ? "同步中..." : "Syncing...")
                        : isActive
                          ? (locale === "zh" ? "重新同步" : "Resync")
                          : (locale === "zh" ? "激活" : "Activate")}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {files.length === 0 && !showCreate ? (
        <div className="card empty-state" style={{ flex: 1 }}>
          <div className="empty-icon"><FileText size={28} style={{ color: "var(--text-muted)" }} /></div>
          <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{i.claudeMd.noFiles}</p>
          <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8, maxWidth: 320 }}>{i.claudeMd.noFilesTip}</p>
          <button className="btn btn-primary btn-sm" style={{ marginTop: 16 }} onClick={() => setShowCreate(true)}>
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

          {/* New File Panel */}
          {showCreate && (
            <div className="section-card" style={{ marginBottom: 16 }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}>
                <h3 style={{ fontSize: 16, fontWeight: 700 }}>{i.claudeMd.newFile}</h3>
                <button className="btn btn-ghost btn-icon-sm" onClick={() => setShowCreate(false)}><X size={16} /></button>
              </div>
              <div style={{ marginBottom: 16 }}>
                <span className="field-label">{i.claudeMd.createIn}</span>
                <input
                  className="input"
                  style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 13 }}
                  placeholder={locale === "zh" ? "输入项目目录路径" : "Enter project directory path"}
                  value={newDirPath}
                  onChange={(e) => setNewDirPath(e.target.value)}
                />
              </div>
              <span className="field-label">{i.claudeMd.selectTemplate}</span>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))", gap: 8 }}>
                {templates.map((tmpl) => (
                  <div
                    key={tmpl.id}
                    className="card card-interactive"
                    style={{ padding: "14px 18px" }}
                    onClick={() => handleCreate(tmpl)}
                  >
                    <div style={{ fontSize: 13, fontWeight: 600 }}>{tmpl.name}</div>
                    <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>{tmpl.file_name}</div>
                    <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>{tmpl.description}</div>
                  </div>
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
              <div
                key={file.path}
                className="card"
                style={{ padding: "16px 20px", opacity: file.disabled ? 0.55 : 1, transition: "opacity 0.2s" }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                  <div className="icon-box" style={{ background: "var(--bg-elevated)", width: 36, height: 36, borderRadius: 6 }}>
                    <FileText size={16} style={{ color: file.disabled ? "var(--text-muted)" : "var(--text-secondary)" }} />
                  </div>
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      <span style={{ fontSize: 13, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {file.project_name}
                      </span>
                      {file.disabled && (
                        <span className="badge badge-muted" style={{ fontSize: 10, padding: "1px 6px" }}>
                          {i.claudeMd.disabled}
                        </span>
                      )}
                      <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{formatSize(file.size_bytes)}</span>
                      {file.modified_at && (
                        <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{file.modified_at}</span>
                      )}
                    </div>
                    <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 3, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {file.tool_name} · {file.file_name} · {file.scope === "project" ? "Project" : "Global"}
                    </div>
                  </div>

                  {/* Actions */}
                  <div style={{ display: "flex", alignItems: "center", gap: 10, flexShrink: 0 }}>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={() => openEditor(file)}
                    >
                      <Pencil size={13} />{i.common.edit}
                    </button>

                    {/* Switch toggle */}
                    <button
                      onClick={() => handleToggle(file)}
                      disabled={togglingPath === file.path}
                      title={file.disabled ? i.claudeMd.enable : i.claudeMd.disable}
                      style={{
                        position: "relative",
                        width: 40,
                        height: 22,
                        borderRadius: 11,
                        border: "none",
                        cursor: togglingPath === file.path ? "wait" : "pointer",
                        background: file.disabled ? "var(--border-strong)" : "var(--success)",
                        transition: "background 0.2s",
                        padding: 0,
                        flexShrink: 0,
                      }}
                    >
                      <span style={{
                        position: "absolute",
                        top: 2,
                        left: file.disabled ? 2 : 20,
                        width: 18,
                        height: 18,
                        borderRadius: "50%",
                        background: "#fff",
                        transition: "left 0.2s",
                        boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
                      }} />
                    </button>

                    <button
                      className="btn btn-ghost btn-icon-sm"
                      title={i.claudeMd.delete}
                      onClick={() => setConfirmDelete(file)}
                    >
                      <Trash2 size={14} style={{ color: "var(--danger)" }} />
                    </button>
                  </div>
                </div>
              </div>
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
