import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Webhook, Plus, Edit3, Trash2, X, Save, FolderOpen } from "lucide-react";
import { t, tReplace } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";

interface Hook {
  id: string; event: string; matcher: string | null;
  command: string; scope: string; project_path: string | null;
  source_event: string | null; source_index: number | null;
  enabled: boolean; timeout: number | null;
}

const HOOK_EVENTS = ["PreToolUse", "PostToolUse", "Notification", "Stop", "SubagentStop"];

export default function Hooks() {
  const [hooks, setHooks] = useState<Hook[]>([]);
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState(false);
  const [editIndex, setEditIndex] = useState<number | null>(null); // null = new
  const [editEvent, setEditEvent] = useState(HOOK_EVENTS[0]);
  const [editMatcher, setEditMatcher] = useState("");
  const [editCommand, setEditCommand] = useState("");
  const [editTimeout, setEditTimeout] = useState("");
  const [editScope, setEditScope] = useState<"global" | "project">("global");
  const [editProjectPath, setEditProjectPath] = useState("");
  const [editOriginalEvent, setEditOriginalEvent] = useState("");
  const [editOriginalIndex, setEditOriginalIndex] = useState<number | null>(null);
  const [editOriginalScope, setEditOriginalScope] = useState<"global" | "project">("global");
  const [editOriginalProjectPath, setEditOriginalProjectPath] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<Hook | null>(null);
  const [saving, setSaving] = useState(false);
  const i = t();

  useEffect(() => { load(); }, []);

  async function load() {
    setLoading(true);
    try { setHooks(await invoke<Hook[]>("scan_hooks")); }
    catch (e) { console.error(e); }
    finally { setLoading(false); }
  }

  function startCreate() {
    setEditing(true);
    setEditIndex(null);
    setEditEvent(HOOK_EVENTS[0]);
    setEditMatcher("");
    setEditCommand("");
    setEditTimeout("");
    setEditScope("global");
    setEditProjectPath("");
    setEditOriginalEvent("");
    setEditOriginalIndex(null);
    setEditOriginalScope("global");
    setEditOriginalProjectPath(null);
  }

  function startEdit(hook: Hook) {
    setEditing(true);
    setEditEvent(hook.event);
    setEditMatcher(hook.matcher || "");
    setEditCommand(hook.command);
    setEditTimeout(hook.timeout ? String(hook.timeout) : "");
    setEditScope(hook.scope === "project" ? "project" : "global");
    setEditProjectPath(hook.project_path || "");
    setEditOriginalEvent(hook.source_event || hook.event);
    setEditOriginalIndex(hook.source_index ?? null);
    setEditOriginalScope(hook.scope === "project" ? "project" : "global");
    setEditOriginalProjectPath(hook.project_path || null);
    setEditIndex(hook.source_index ?? 0);
  }

  function cancelEdit() {
    setEditing(false);
    setEditIndex(null);
  }

  async function handleSave() {
    if (!editCommand.trim()) {
      showToast("error", i.hooks.commandRequired);
      return;
    }
    if (editScope === "project" && !editProjectPath.trim()) {
      showToast("error", i.hooks.projectPathRequired);
      return;
    }
    setSaving(true);
    try {
      const targetProjectPath = editScope === "project" ? editProjectPath.trim() : null;
      if (editOriginalIndex !== null && editOriginalEvent) {
        const sourceChanged =
          editOriginalEvent !== editEvent
          || editOriginalScope !== editScope
          || (editOriginalProjectPath || null) !== targetProjectPath;

        if (sourceChanged) {
          await invoke("delete_hook_from_settings", {
            event: editOriginalEvent,
            index: editOriginalIndex,
            scope: editOriginalScope,
            projectPath: editOriginalProjectPath,
          });
          await invoke("save_hook_to_settings", {
            event: editEvent,
            matcher: editMatcher.trim() || null,
            command: editCommand.trim(),
            timeout: editTimeout.trim() ? parseInt(editTimeout.trim(), 10) : null,
            scope: editScope,
            projectPath: targetProjectPath,
            editIndex: null,
          });
          if (
            editOriginalProjectPath
            && targetProjectPath
            && editOriginalScope === "project"
            && editScope === "project"
            && editOriginalProjectPath !== targetProjectPath
          ) {
            await invoke("remap_imported_project_root", {
              sourcePath: editOriginalProjectPath,
              targetPath: targetProjectPath,
            });
          }
        } else {
          await invoke("save_hook_to_settings", {
            event: editEvent,
            matcher: editMatcher.trim() || null,
            command: editCommand.trim(),
            timeout: editTimeout.trim() ? parseInt(editTimeout.trim(), 10) : null,
            scope: editScope,
            projectPath: targetProjectPath,
            editIndex: editOriginalIndex,
          });
        }
      } else {
        // Creating new
        await invoke("save_hook_to_settings", {
          event: editEvent,
          matcher: editMatcher.trim() || null,
          command: editCommand.trim(),
          timeout: editTimeout.trim() ? parseInt(editTimeout.trim(), 10) : null,
          scope: editScope,
          projectPath: targetProjectPath,
          editIndex: null,
        });
      }
      showToast("success", i.hooks.saveSuccess);
      setEditing(false);
      setEditIndex(null);
      await load();
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setSaving(false);
    }
  }

  async function doDelete(hook: Hook) {
    const event = hook.source_event || hook.event;
    const index = hook.source_index;
    if (index === null || index === undefined) {
      showToast("error", i.hooks.hookMetaInvalid);
      return;
    }
    try {
      await invoke("delete_hook_from_settings", {
        event,
        index,
        scope: hook.scope,
        projectPath: hook.project_path,
      });
      showToast("success", i.hooks.deleteSuccess);
      await load();
    } catch (e) {
      showToast("error", String(e));
    }
  }

  if (loading) {
    return <div className="loading-center"><div className="spinner" /><span style={{ fontSize: 13, color: "var(--text-muted)" }}>{i.hooks.loading}</span></div>;
  }

  // Edit / Create form
  if (editing) {
    return (
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <div className="page-header">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <button className="btn btn-ghost btn-icon-sm" onClick={cancelEdit}><X size={16} /></button>
            <h2 className="page-title">{editIndex !== null ? i.hooks.editHook : i.hooks.createHook}</h2>
          </div>
        </div>

        <div style={{ flex: 1, overflowY: "auto", padding: "0 4px" }}>
          <div className="section-card" style={{ maxWidth: 600 }}>
            {/* Event */}
            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.event}</label>
              <select className="input" value={editEvent} onChange={e => setEditEvent(e.target.value)}>
                {HOOK_EVENTS.map(ev => <option key={ev} value={ev}>{ev}</option>)}
              </select>
            </div>

            {/* Matcher */}
            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.matcher}</label>
              <input className="input" value={editMatcher} onChange={e => setEditMatcher(e.target.value)}
                placeholder={i.hooks.matcherPlaceholder} />
            </div>

            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.scope}</label>
              <select className="input" value={editScope} onChange={e => setEditScope(e.target.value as "global" | "project")}>
                <option value="global">{i.hooks.global}</option>
                <option value="project">{i.hooks.project}</option>
              </select>
            </div>

            {editScope === "project" && (
              <div style={{ marginBottom: 20 }}>
                <label className="field-label">{i.hooks.projectPath}</label>
                <div style={{ display: "flex", gap: 8 }}>
                  <input
                    className="input"
                    value={editProjectPath}
                    onChange={e => setEditProjectPath(e.target.value)}
                    placeholder={i.hooks.projectPathPlaceholder}
                    style={{ fontFamily: "'JetBrains Mono', monospace" }}
                  />
                  <button
                    className="btn btn-secondary btn-icon-sm"
                    onClick={async () => {
                      try {
                        const picked = await invoke<string | null>("pick_folder");
                        if (picked) setEditProjectPath(picked);
                      } catch (e) { console.error(e); }
                    }}
                    type="button"
                    title={i.hooks.projectPath}
                  >
                    <FolderOpen size={14} />
                  </button>
                </div>
              </div>
            )}

            {/* Command */}
            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.command}</label>
              <input className="input" value={editCommand} onChange={e => setEditCommand(e.target.value)}
                placeholder={i.hooks.commandPlaceholder} style={{ fontFamily: "'JetBrains Mono', monospace" }} />
            </div>

            {/* Timeout */}
            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.timeout}</label>
              <input className="input" type="number" value={editTimeout} onChange={e => setEditTimeout(e.target.value)}
                placeholder={i.hooks.timeoutPlaceholder} />
            </div>
          </div>
        </div>

        <div style={{ display: "flex", gap: 10, justifyContent: "flex-end", padding: "16px 4px 0" }}>
          <button className="btn btn-secondary btn-sm" onClick={cancelEdit}>{i.common.cancel}</button>
          <button className="btn btn-primary btn-sm" onClick={handleSave} disabled={saving} style={{ gap: 6 }}>
            <Save size={14} />{i.common.save}
          </button>
        </div>
      </div>
    );
  }

  // List view
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.hooks.title}</h2>
          <p className="page-subtitle">{tReplace(i.hooks.hookCount, { count: hooks.length })}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-primary btn-sm" onClick={startCreate} style={{ gap: 6 }}>
            <Plus size={14} />{i.hooks.newHook}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={load}><RefreshCw size={14} />{i.common.refresh}</button>
        </div>
      </div>

      {hooks.length === 0 ? (
        <div className="card empty-state" style={{ flex: 1 }}>
          <div className="empty-icon"><Webhook size={28} style={{ color: "var(--text-muted)" }} /></div>
          <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{i.hooks.noHooks}</p>
          <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8, maxWidth: 320 }}>{i.hooks.noHooksTip}</p>
        </div>
      ) : (
        <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 10 }} className="stagger">
          {hooks.map((hook) => (
            <div key={hook.id} className="card card-hover" style={{ padding: "20px 24px" }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
                  <span className="dot dot-active" />
                  <span className="badge badge-accent" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 11 }}>{hook.event}</span>
                  {hook.matcher && (
                    <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
                      {i.hooks.matcher}: <span style={{ fontFamily: "'JetBrains Mono', monospace" }}>{hook.matcher}</span>
                    </span>
                  )}
                  {hook.timeout && (
                    <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                      {i.hooks.timeout}: {hook.timeout}ms
                    </span>
                  )}
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span className="badge badge-muted">{hook.scope === "global" ? i.hooks.global : i.hooks.project}</span>
                  <button className="btn btn-ghost btn-icon-sm" onClick={() => startEdit(hook)} title={i.hooks.editHook}>
                    <Edit3 size={14} />
                  </button>
                  <button className="btn btn-ghost btn-icon-sm" onClick={() => setPendingDelete(hook)} title={i.common.delete}
                    style={{ color: "var(--danger)" }}>
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
              <div className="code-block" style={{ marginTop: 14 }}>{hook.command}</div>
              {hook.project_path && (
                <p style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace", color: "var(--text-muted)", marginTop: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {hook.project_path}
                </p>
              )}
            </div>
          ))}
        </div>
      )}

      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={i.hooks.deleteConfirm}
        message={i.hooks.deleteConfirmDesc}
        variant="destructive"
        onConfirm={() => { if (pendingDelete) doDelete(pendingDelete); setPendingDelete(null); }}
        onCancel={() => setPendingDelete(null)}
      />
    </div>
  );
}
