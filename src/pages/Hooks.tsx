import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FolderOpen, Plus, RefreshCw, Save, Webhook, X } from "lucide-react";
import { t, tReplace } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import HookCard, { type HookCardHook } from "../components/HookCard";
import EmptyState from "../components/states/EmptyState";
import ErrorState from "../components/states/ErrorState";
import LoadingState from "../components/states/LoadingState";
import { queryKeys } from "../hooks/queries";
import { useDeleteHookFromSettingsMutation, useUpdateHookInSettingsMutation } from "../hooks/mutations";

type Hook = HookCardHook;

const HOOK_EVENTS = ["PreToolUse", "PostToolUse", "Notification", "Stop", "SubagentStop"];

export default function Hooks() {
  const queryClient = useQueryClient();
  const [hooks, setHooks] = useState<Hook[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
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
  const updateHookInSettingsMutation = useUpdateHookInSettingsMutation();
  const deleteHookFromSettingsMutation = useDeleteHookFromSettingsMutation();

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setHooks(
        await queryClient.fetchQuery({
          queryKey: queryKeys.hooks,
          queryFn: () => invoke<Hook[]>("scan_hooks"),
          staleTime: 30_000,
        }),
      );
    } catch (e) {
      setLoadError(String(e));
    } finally {
      setLoading(false);
    }
  }, [queryClient]);

  useEffect(() => {
    void load();
  }, [load]);

  const startCreate = useCallback(() => {
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
  }, []);

  const startEdit = useCallback((hook: Hook) => {
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
  }, []);

  const cancelEdit = useCallback(() => {
    setEditing(false);
    setEditIndex(null);
  }, []);

  const handleSave = useCallback(async () => {
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
      const nextHook = {
        event: editEvent,
        matcher: editMatcher.trim() || null,
        command: editCommand.trim(),
        timeout: editTimeout.trim() ? parseInt(editTimeout.trim(), 10) : null,
        scope: editScope,
        projectPath: targetProjectPath,
        editIndex: null,
      };

      if (editOriginalIndex !== null && editOriginalEvent) {
        const sourceChanged =
          editOriginalEvent !== editEvent ||
          editOriginalScope !== editScope ||
          (editOriginalProjectPath || null) !== targetProjectPath;

        if (sourceChanged) {
          const updatedHooks = await updateHookInSettingsMutation.mutateAsync({
            previous: {
              event: editOriginalEvent,
              index: editOriginalIndex,
              scope: editOriginalScope,
              projectPath: editOriginalProjectPath,
            },
            next: nextHook,
            remapProjectRoot:
              editOriginalProjectPath &&
              targetProjectPath &&
              editOriginalScope === "project" &&
              editScope === "project" &&
              editOriginalProjectPath !== targetProjectPath
                ? { sourcePath: editOriginalProjectPath, targetPath: targetProjectPath }
                : null,
          });
          setHooks(updatedHooks);
        } else {
          const updatedHooks = await updateHookInSettingsMutation.mutateAsync({
            previous: null,
            next: { ...nextHook, editIndex: editOriginalIndex },
          });
          setHooks(updatedHooks);
        }
      } else {
        const updatedHooks = await updateHookInSettingsMutation.mutateAsync({
          previous: null,
          next: nextHook,
        });
        setHooks(updatedHooks);
      }
      showToast("success", i.hooks.saveSuccess);
      setEditing(false);
      setEditIndex(null);
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setSaving(false);
    }
  }, [
    editCommand,
    editEvent,
    editMatcher,
    editOriginalEvent,
    editOriginalIndex,
    editOriginalProjectPath,
    editOriginalScope,
    editProjectPath,
    editScope,
    editTimeout,
    i.hooks.commandRequired,
    i.hooks.projectPathRequired,
    i.hooks.saveSuccess,
    updateHookInSettingsMutation,
  ]);

  const doDelete = useCallback(
    async (hook: Hook) => {
      const event = hook.source_event || hook.event;
      const index = hook.source_index;
      if (index === null || index === undefined) {
        showToast("error", i.hooks.hookMetaInvalid);
        return;
      }
      try {
        const updatedHooks = await deleteHookFromSettingsMutation.mutateAsync({
          event,
          index,
          scope: hook.scope === "project" ? "project" : "global",
          projectPath: hook.project_path,
        });
        setHooks(updatedHooks);
        showToast("success", i.hooks.deleteSuccess);
      } catch (e) {
        showToast("error", String(e));
      }
    },
    [deleteHookFromSettingsMutation, i.hooks.deleteSuccess, i.hooks.hookMetaInvalid],
  );

  const handleDeleteClick = useCallback((hook: Hook) => {
    setPendingDelete(hook);
  }, []);

  const handlePickProjectPath = useCallback(async () => {
    try {
      const picked = await invoke<string | null>("pick_folder");
      if (picked) setEditProjectPath(picked);
    } catch (e) {
      console.error(e);
    }
  }, []);

  const handlePickProjectPathClick = useCallback(() => {
    void handlePickProjectPath();
  }, [handlePickProjectPath]);

  const handleRefresh = useCallback(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const handleSaveShortcut = () => {
      if (editing && !saving) {
        void handleSave();
      }
    };
    const handleNewShortcut = () => {
      if (!editing) {
        startCreate();
      }
    };
    const handleEscapeShortcut = () => {
      if (editing) {
        cancelEdit();
      }
    };

    window.addEventListener("cchub-shortcut-save", handleSaveShortcut);
    window.addEventListener("cchub-shortcut-new", handleNewShortcut);
    window.addEventListener("cchub-shortcut-escape", handleEscapeShortcut);
    return () => {
      window.removeEventListener("cchub-shortcut-save", handleSaveShortcut);
      window.removeEventListener("cchub-shortcut-new", handleNewShortcut);
      window.removeEventListener("cchub-shortcut-escape", handleEscapeShortcut);
    };
  }, [cancelEdit, editing, handleSave, saving, startCreate]);

  if (loading) {
    return <LoadingState label={i.hooks.loading} />;
  }

  // Edit / Create form
  if (editing) {
    return (
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <div className="page-header">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <button
              className="btn btn-ghost btn-icon-sm"
              aria-label={i.common.close}
              title={i.common.close}
              onClick={cancelEdit}
            >
              <X size={16} />
            </button>
            <h2 className="page-title">{editIndex !== null ? i.hooks.editHook : i.hooks.createHook}</h2>
          </div>
        </div>

        <div style={{ flex: 1, overflowY: "auto", padding: "0 4px" }}>
          <div className="section-card" style={{ maxWidth: 600 }}>
            {/* Event */}
            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.event}</label>
              <select className="input" value={editEvent} onChange={(e) => setEditEvent(e.target.value)}>
                {HOOK_EVENTS.map((ev) => (
                  <option key={ev} value={ev}>
                    {ev}
                  </option>
                ))}
              </select>
            </div>

            {/* Matcher */}
            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.matcher}</label>
              <input
                className="input"
                value={editMatcher}
                onChange={(e) => setEditMatcher(e.target.value)}
                placeholder={i.hooks.matcherPlaceholder}
              />
            </div>

            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.scope}</label>
              <select
                className="input"
                value={editScope}
                onChange={(e) => setEditScope(e.target.value as "global" | "project")}
              >
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
                    onChange={(e) => setEditProjectPath(e.target.value)}
                    placeholder={i.hooks.projectPathPlaceholder}
                    style={{ fontFamily: "'JetBrains Mono', monospace" }}
                  />
                  <button
                    className="btn btn-secondary btn-icon-sm"
                    onClick={handlePickProjectPathClick}
                    type="button"
                    aria-label={i.hooks.projectPath}
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
              <input
                className="input"
                value={editCommand}
                onChange={(e) => setEditCommand(e.target.value)}
                placeholder={i.hooks.commandPlaceholder}
                style={{ fontFamily: "'JetBrains Mono', monospace" }}
              />
            </div>

            {/* Timeout */}
            <div style={{ marginBottom: 20 }}>
              <label className="field-label">{i.hooks.timeout}</label>
              <input
                className="input"
                type="number"
                value={editTimeout}
                onChange={(e) => setEditTimeout(e.target.value)}
                placeholder={i.hooks.timeoutPlaceholder}
              />
            </div>
          </div>
        </div>

        <div style={{ display: "flex", gap: 10, justifyContent: "flex-end", padding: "16px 4px 0" }}>
          <button className="btn btn-secondary btn-sm" onClick={cancelEdit}>
            {i.common.cancel}
          </button>
          <button className="btn btn-primary btn-sm" onClick={handleSave} disabled={saving} style={{ gap: 6 }}>
            <Save size={14} />
            {i.common.save}
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
            <Plus size={14} />
            {i.hooks.newHook}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={handleRefresh}>
            <RefreshCw size={14} />
            {i.common.refresh}
          </button>
        </div>
      </div>

      {loadError ? (
        <ErrorState title={i.hooks.title} message={loadError} retryLabel={i.common.refresh} onRetry={handleRefresh} />
      ) : hooks.length === 0 ? (
        <EmptyState
          title={i.hooks.noHooks}
          description={i.hooks.noHooksTip}
          icon={<Webhook size={28} style={{ color: "var(--text-muted)" }} />}
        />
      ) : (
        <div
          style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 10 }}
          className="stagger"
        >
          {hooks.map((hook) => (
            <HookCard
              key={hook.id}
              hook={hook}
              matcherLabel={i.hooks.matcher}
              timeoutLabel={i.hooks.timeout}
              globalLabel={i.hooks.global}
              projectLabel={i.hooks.project}
              editTitle={i.hooks.editHook}
              deleteTitle={i.common.delete}
              onEdit={startEdit}
              onDelete={handleDeleteClick}
            />
          ))}
        </div>
      )}

      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={i.hooks.deleteConfirm}
        message={i.hooks.deleteConfirmDesc}
        variant="destructive"
        onConfirm={() => {
          if (pendingDelete) void doDelete(pendingDelete);
          setPendingDelete(null);
        }}
        onCancel={() => setPendingDelete(null)}
      />
    </div>
  );
}
