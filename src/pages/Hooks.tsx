import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Webhook, Plus, Edit3, Trash2, X, Save } from "lucide-react";
import { t, tReplace } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";

interface Hook {
  id: string; event: string; matcher: string | null;
  command: string; scope: string; project_path: string | null;
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
  const [editOriginalEvent, setEditOriginalEvent] = useState(""); // for locating hook in settings
  const [editOriginalIndex, setEditOriginalIndex] = useState<number | null>(null);
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
    setEditOriginalEvent("");
    setEditOriginalIndex(null);
  }

  function startEdit(hook: Hook) {
    setEditing(true);
    setEditEvent(hook.event);
    setEditMatcher(hook.matcher || "");
    setEditCommand(hook.command);
    setEditTimeout(hook.timeout ? String(hook.timeout) : "");
    // Parse the hook id to get the original event and index
    const dashIdx = hook.id.lastIndexOf("-");
    const origEvent = hook.id.substring(0, dashIdx);
    const origIndex = parseInt(hook.id.substring(dashIdx + 1), 10);
    setEditOriginalEvent(origEvent);
    setEditOriginalIndex(origIndex);
    setEditIndex(origIndex);
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
    setSaving(true);
    try {
      if (editOriginalIndex !== null && editOriginalEvent) {
        // Editing existing: if event changed, delete old then create new
        if (editOriginalEvent !== editEvent) {
          await invoke("delete_hook_from_settings", {
            event: editOriginalEvent,
            index: editOriginalIndex,
            scope: "global",
            projectPath: null,
          });
          await invoke("save_hook_to_settings", {
            event: editEvent,
            matcher: editMatcher.trim() || null,
            command: editCommand.trim(),
            timeout: editTimeout.trim() ? parseInt(editTimeout.trim(), 10) : null,
            scope: "global",
            projectPath: null,
            editIndex: null,
          });
        } else {
          await invoke("save_hook_to_settings", {
            event: editEvent,
            matcher: editMatcher.trim() || null,
            command: editCommand.trim(),
            timeout: editTimeout.trim() ? parseInt(editTimeout.trim(), 10) : null,
            scope: "global",
            projectPath: null,
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
          scope: "global",
          projectPath: null,
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
    const dashIdx = hook.id.lastIndexOf("-");
    const event = hook.id.substring(0, dashIdx);
    const index = parseInt(hook.id.substring(dashIdx + 1), 10);
    try {
      await invoke("delete_hook_from_settings", {
        event,
        index,
        scope: "global",
        projectPath: null,
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
