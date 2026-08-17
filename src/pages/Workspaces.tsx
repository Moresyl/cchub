import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FolderOpen, Layers, Plus, X } from "lucide-react";
import { getLocale } from "../lib/i18n";
import ConfirmDialog from "../components/ConfirmDialog";
import WorkspaceCard, { type WorkspaceCardWorkspace } from "../components/WorkspaceCard";
import ProjectProfilePanel from "../components/ProjectProfilePanel";
import EmptyState from "../components/states/EmptyState";
import ErrorState from "../components/states/ErrorState";
import LoadingState from "../components/states/LoadingState";
import { queryKeys } from "../hooks/queries";
import {
  useCreateWorkspaceMutation,
  useDeleteWorkspaceMutation,
  useSwitchWorkspaceMutation,
  useUpdateWorkspaceMutation,
} from "../hooks/mutations";

type Workspace = WorkspaceCardWorkspace;

function sortWorkspaces(workspaces: Workspace[]) {
  return [...workspaces].sort((left, right) => left.name.localeCompare(right.name));
}

export default function Workspaces() {
  const queryClient = useQueryClient();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [newPath, setNewPath] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [editDesc, setEditDesc] = useState("");
  const [editPath, setEditPath] = useState("");
  const [pendingDelete, setPendingDelete] = useState<Workspace | null>(null);
  const locale = getLocale();
  const createWorkspaceMutation = useCreateWorkspaceMutation();
  const switchWorkspaceMutation = useSwitchWorkspaceMutation();
  const deleteWorkspaceMutation = useDeleteWorkspaceMutation();
  const updateWorkspaceMutation = useUpdateWorkspaceMutation();
  const uiText = (zhText: string, enText: string, jaText?: string) =>
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setWorkspaces(
        await queryClient.fetchQuery({
          queryKey: queryKeys.workspaces,
          queryFn: () => invoke<Workspace[]>("get_workspaces"),
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

  const handleCreate = useCallback(async () => {
    if (!newName.trim()) return;
    try {
      const created = await createWorkspaceMutation.mutateAsync({
        name: newName.trim(),
        description: newDesc.trim() || null,
        basePath: newPath.trim() || null,
      });
      setWorkspaces((current) => sortWorkspaces([...current, created]));
      setShowCreate(false);
      setNewName("");
      setNewDesc("");
      setNewPath("");
    } catch (e) {
      console.error(e);
    }
  }, [createWorkspaceMutation, newDesc, newName, newPath]);

  const handleSwitch = useCallback(
    async (id: string) => {
      try {
        await switchWorkspaceMutation.mutateAsync({ id });
        window.dispatchEvent(new Event("cchub-project-profile-refresh"));
        setWorkspaces((current) =>
          current.map((workspace) => ({
            ...workspace,
            is_active: workspace.id === id,
          })),
        );
      } catch (e) {
        console.error(e);
      }
    },
    [switchWorkspaceMutation],
  );

  const handleDelete = useCallback((ws: Workspace) => {
    if (ws.is_active) return;
    setPendingDelete(ws);
  }, []);

  const doDelete = useCallback(
    async (ws: Workspace) => {
      try {
        await deleteWorkspaceMutation.mutateAsync({ id: ws.id });
        setWorkspaces((current) => current.filter((workspace) => workspace.id !== ws.id));
      } catch (e) {
        console.error(e);
      }
    },
    [deleteWorkspaceMutation],
  );

  const handleSaveEdit = useCallback(
    async (ws: Workspace) => {
      try {
        await updateWorkspaceMutation.mutateAsync({
          id: ws.id,
          name: editName.trim(),
          description: editDesc.trim() || null,
          basePath: editPath.trim() || null,
        });
        setWorkspaces((current) =>
          sortWorkspaces(
            current.map((workspace) =>
              workspace.id === ws.id
                ? {
                    ...workspace,
                    name: editName.trim(),
                    description: editDesc.trim() || null,
                    base_path: editPath.trim() || null,
                  }
                : workspace,
            ),
          ),
        );
        setEditing(null);
      } catch (e) {
        console.error(e);
      }
    },
    [editDesc, editName, editPath, updateWorkspaceMutation],
  );

  const startEdit = useCallback((ws: Workspace) => {
    setEditing(ws.id);
    setEditName(ws.name);
    setEditDesc(ws.description || "");
    setEditPath(ws.base_path || "");
  }, []);

  const closeCreate = useCallback(() => {
    setShowCreate(false);
    setNewName("");
    setNewDesc("");
    setNewPath("");
  }, []);

  const cancelEdit = useCallback(() => {
    setEditing(null);
  }, []);

  const openCreate = useCallback(() => {
    setShowCreate(true);
  }, []);

  const handlePickCreatePath = useCallback(async () => {
    try {
      const picked = await invoke<string | null>("pick_folder");
      if (picked) setNewPath(picked);
    } catch (e) {
      console.error(e);
    }
  }, []);

  const handlePickEditPath = useCallback(async () => {
    try {
      const picked = await invoke<string | null>("pick_folder");
      if (picked) setEditPath(picked);
    } catch (e) {
      console.error(e);
    }
  }, []);

  const handlePickCreatePathClick = useCallback(() => {
    void handlePickCreatePath();
  }, [handlePickCreatePath]);

  const handlePickEditPathClick = useCallback(() => {
    void handlePickEditPath();
  }, [handlePickEditPath]);

  const editingWorkspace = editing ? (workspaces.find((ws) => ws.id === editing) ?? null) : null;

  useEffect(() => {
    const handleSaveShortcut = () => {
      if (showCreate) {
        void handleCreate();
        return;
      }
      if (editingWorkspace) {
        void handleSaveEdit(editingWorkspace);
      }
    };
    const handleNewShortcut = () => {
      if (!showCreate && !editingWorkspace) {
        openCreate();
      }
    };
    const handleEscapeShortcut = () => {
      if (showCreate) {
        closeCreate();
        return;
      }
      if (editingWorkspace) {
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
  }, [cancelEdit, closeCreate, editingWorkspace, handleCreate, handleSaveEdit, openCreate, showCreate]);

  if (loading) {
    return <LoadingState label={uiText("加载中...", "Loading...", "読み込み中...")} />;
  }

  const activeWs = workspaces.find((w) => w.is_active);

  return (
    <div className="animate-in">
      <div className="page-header">
        <div>
          <h2 className="page-title">{uiText("工作区", "Workspaces", "ワークスペース")}</h2>
          <p className="page-subtitle">
            {uiText(
              `${workspaces.length} 个工作区${activeWs ? `，当前: ${activeWs.name}` : ""}`,
              `${workspaces.length} workspaces${activeWs ? `, active: ${activeWs.name}` : ""}`,
              `${workspaces.length} 個のワークスペース${activeWs ? `、現在: ${activeWs.name}` : ""}`,
            )}
          </p>
        </div>
        <button className="btn btn-primary btn-sm" onClick={openCreate} style={{ gap: 6 }}>
          <Plus size={14} />
          {uiText("新建", "New", "新規")}
        </button>
      </div>

      <ProjectProfilePanel />

      {loadError ? (
        <ErrorState
          title={uiText("加载工作区失败", "Failed to load workspaces", "ワークスペースの読み込みに失敗しました")}
          message={loadError}
          retryLabel={uiText("重试", "Retry", "再試行")}
          onRetry={() => {
            void load();
          }}
        />
      ) : (
        <>
          {/* Create Form */}
          {showCreate && (
            <div className="section-card" style={{ marginBottom: 20 }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
                <h3 style={{ fontSize: 14, fontWeight: 600 }}>
                  {uiText("新建工作区", "New Workspace", "ワークスペースを新規作成")}
                </h3>
                <button className="btn btn-ghost btn-icon-sm" onClick={closeCreate}>
                  <X size={14} />
                </button>
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                <input
                  className="input"
                  placeholder={uiText("工作区名称", "Workspace name", "ワークスペース名")}
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                />
                <input
                  className="input"
                  placeholder={uiText("描述（可选）", "Description (optional)", "説明（任意）")}
                  value={newDesc}
                  onChange={(e) => setNewDesc(e.target.value)}
                />
                <div style={{ display: "flex", gap: 8 }}>
                  <input
                    className="input"
                    placeholder={uiText("项目路径（可选）", "Project path (optional)", "プロジェクトパス（任意）")}
                    value={newPath}
                    onChange={(e) => setNewPath(e.target.value)}
                    style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
                  />
                  <button className="btn btn-secondary btn-sm" onClick={handlePickCreatePathClick} type="button">
                    <FolderOpen size={14} />
                  </button>
                </div>
                <button
                  className="btn btn-primary btn-sm"
                  onClick={handleCreate}
                  disabled={!newName.trim()}
                  style={{ alignSelf: "flex-end" }}
                >
                  <Plus size={14} />
                  {uiText("创建", "Create", "作成")}
                </button>
              </div>
            </div>
          )}

          {/* Workspace List */}
          {workspaces.length === 0 ? (
            <EmptyState
              title={uiText("还没有工作区", "No workspaces yet", "ワークスペースがありません")}
              description={uiText(
                "工作区可以隔离不同项目的 MCP Server、Profile 和配置，点击上方「新建」开始创建。",
                'Workspaces isolate MCP servers, profiles, and configs for different projects. Click "New" above to get started.',
                "ワークスペースでプロジェクトごとに MCP Server、Profile、設定を分離できます。上の「新規」をクリックして作成してください。",
              )}
              icon={<Layers size={28} style={{ color: "var(--text-muted)" }} />}
            />
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }} className="stagger">
              {workspaces.map((ws) => (
                <WorkspaceCard
                  key={ws.id}
                  workspace={ws}
                  isEditing={editing === ws.id}
                  editName={editName}
                  editDesc={editDesc}
                  editPath={editPath}
                  activeLabel={uiText("当前", "Active", "現在")}
                  editTitle={uiText("编辑", "Edit", "編集")}
                  deleteTitle={uiText("删除", "Delete", "削除")}
                  descriptionPlaceholder={uiText("描述", "Description", "説明")}
                  pathPlaceholder={uiText("项目路径", "Project path", "プロジェクトパス")}
                  pickFolderTitle={uiText("选择目录", "Pick folder", "フォルダを選択")}
                  onSwitch={handleSwitch}
                  onStartEdit={startEdit}
                  onDelete={handleDelete}
                  onCancelEdit={cancelEdit}
                  onSaveEdit={handleSaveEdit}
                  onEditNameChange={setEditName}
                  onEditDescChange={setEditDesc}
                  onEditPathChange={setEditPath}
                  onPickFolder={handlePickEditPathClick}
                />
              ))}
            </div>
          )}
        </>
      )}
      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={uiText("删除工作区", "Delete Workspace", "ワークスペースを削除")}
        message={uiText(
          `确定删除工作区「${pendingDelete?.name}」？`,
          `Delete workspace "${pendingDelete?.name}"?`,
          `ワークスペース「${pendingDelete?.name}」を削除しますか？`,
        )}
        confirmText={uiText("删除", "Delete", "削除")}
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
