/* eslint-disable react-hooks/exhaustive-deps */
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Monitor, Plus, Save, Shield, X } from "lucide-react";
import { getLocale } from "../lib/i18n";
import ConfirmDialog from "../components/ConfirmDialog";
import McpClientCard, { type McpClientCardClient } from "../components/McpClientCard";
import McpClientAccessRow, { type McpClientAccessRowServer } from "../components/McpClientAccessRow";
import EmptyState from "../components/states/EmptyState";
import ErrorState from "../components/states/ErrorState";
import LoadingState from "../components/states/LoadingState";
import { queryKeys } from "../hooks/queries";
import {
  useCreateMcpClientMutation,
  useDeleteMcpClientMutation,
  useUpdateMcpClientAccessMutation,
} from "../hooks/mutations";

type McpClient = McpClientCardClient;

type McpServer = McpClientAccessRowServer;

function sortMcpClients(clients: McpClient[]) {
  return [...clients].sort((left, right) => left.name.localeCompare(right.name));
}

export default function McpClients() {
  const queryClient = useQueryClient();
  const [clients, setClients] = useState<McpClient[]>([]);
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState<McpClient | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState("");
  const [newConfigPath, setNewConfigPath] = useState("");
  const [editing, setEditing] = useState(false);
  const [editAccess, setEditAccess] = useState<Record<string, boolean>>({});
  const [pendingDelete, setPendingDelete] = useState<McpClient | null>(null);
  const locale = getLocale();
  const createMcpClientMutation = useCreateMcpClientMutation();
  const deleteMcpClientMutation = useDeleteMcpClientMutation();
  const updateMcpClientAccessMutation = useUpdateMcpClientAccessMutation();
  const uiText = (zhText: string, enText: string, jaText?: string) =>
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const [c, s] = await queryClient.fetchQuery({
        queryKey: queryKeys.mcpClientsPage,
        queryFn: () => Promise.all([invoke<McpClient[]>("get_mcp_clients"), invoke<McpServer[]>("scan_mcp_servers")]),
        staleTime: 30_000,
      });
      setClients(c);
      setServers(s);
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
      const created = await createMcpClientMutation.mutateAsync({
        name: newName.trim(),
        configPath: newConfigPath.trim() || null,
      });
      setClients((current) => sortMcpClients([...current, created]));
      setSelected(created);
      setShowCreate(false);
      setNewName("");
      setNewConfigPath("");
    } catch (e) {
      console.error(e);
    }
  }, [createMcpClientMutation, newConfigPath, newName]);

  const handleDelete = useCallback((client: McpClient) => {
    setPendingDelete(client);
  }, []);

  const doDelete = useCallback(
    async (client: McpClient) => {
      try {
        await deleteMcpClientMutation.mutateAsync({ id: client.id });
        if (selected?.id === client.id) setSelected(null);
        setClients((current) => current.filter((item) => item.id !== client.id));
      } catch (e) {
        console.error(e);
      }
    },
    [deleteMcpClientMutation, selected?.id],
  );

  const startEdit = useCallback(
    (client: McpClient) => {
      setEditing(true);
      const access: Record<string, boolean> = {};
      for (const s of servers) {
        access[s.id] = client.server_access[s.id] ?? true;
      }
      setEditAccess(access);
    },
    [servers],
  );

  const handleSaveAccess = useCallback(async () => {
    if (!selected) return;
    try {
      await updateMcpClientAccessMutation.mutateAsync({ id: selected.id, serverAccess: editAccess });
      const updated = { ...selected, server_access: editAccess };
      setClients((current) => current.map((client) => (client.id === selected.id ? updated : client)));
      setSelected(updated);
      setEditing(false);
    } catch (e) {
      console.error(e);
    }
  }, [editAccess, selected, updateMcpClientAccessMutation]);

  const openCreate = useCallback(() => {
    setShowCreate(true);
  }, []);

  const closeCreate = useCallback(() => {
    setShowCreate(false);
    setNewName("");
    setNewConfigPath("");
  }, []);

  const stopEditing = useCallback(() => {
    setEditing(false);
  }, []);

  const handleSelectClient = useCallback((client: McpClient) => {
    setSelected(client);
    setEditing(false);
  }, []);

  const handleStartEditSelected = useCallback(() => {
    if (!selected) return;
    startEdit(selected);
  }, [selected, startEdit]);

  const handleToggleServerAccess = useCallback((serverId: string) => {
    setEditAccess((prev) => ({ ...prev, [serverId]: !(prev[serverId] ?? true) }));
  }, []);

  const clearSelectedClient = useCallback(() => {
    setSelected(null);
  }, []);

  useEffect(() => {
    const handleSaveShortcut = () => {
      if (showCreate) {
        void handleCreate();
        return;
      }
      if (editing) {
        void handleSaveAccess();
      }
    };
    const handleNewShortcut = () => {
      if (!showCreate && !editing) {
        openCreate();
      }
    };
    const handleEscapeShortcut = () => {
      if (showCreate) {
        closeCreate();
        return;
      }
      if (editing) {
        stopEditing();
        return;
      }
      if (selected) {
        clearSelectedClient();
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
  }, [clearSelectedClient, editing, handleCreate, handleSaveAccess, openCreate, selected, showCreate, stopEditing]);

  if (loading) {
    return <LoadingState label={uiText("加载中...", "Loading...", "読み込み中...")} />;
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{uiText("MCP 客户端", "MCP Clients", "MCP クライアント")}</h2>
          <p className="page-subtitle">
            {uiText(
              `管理 ${clients.length} 个 AI 客户端应用的 MCP 访问权限`,
              `Manage MCP access for ${clients.length} AI client apps`,
              `${clients.length} 個の AI クライアントの MCP アクセス権を管理`,
            )}
          </p>
        </div>
        <button className="btn btn-primary btn-sm" onClick={openCreate} style={{ gap: 6 }}>
          <Plus size={14} />
          {uiText("添加客户端", "Add Client", "クライアントを追加")}
        </button>
      </div>

      {loadError ? (
        <ErrorState
          title={uiText(
            "加载 MCP 客户端失败",
            "Failed to load MCP clients",
            "MCP クライアントの読み込みに失敗しました",
          )}
          message={loadError}
          retryLabel={uiText("重试", "Retry", "再試行")}
          onRetry={() => {
            void load();
          }}
        />
      ) : clients.length === 0 && !showCreate ? (
        <EmptyState
          title={uiText("尚未添加客户端", "No clients added", "まだクライアントがありません")}
          description={uiText(
            "添加 AI 客户端应用以管理其对 MCP 服务器的访问权限",
            "Add AI client apps to manage their MCP server access",
            "AI クライアントを追加して MCP サーバーへのアクセス権を管理します",
          )}
          icon={<Monitor size={28} style={{ color: "var(--text-muted)" }} />}
          action={
            <button className="btn btn-primary btn-sm" type="button" onClick={openCreate}>
              <Plus size={14} />
              {uiText("添加客户端", "Add Client", "クライアントを追加")}
            </button>
          }
        />
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 24, flex: 1, minHeight: 0 }}>
          {/* Client List */}
          <div style={{ overflowY: "auto", display: "flex", flexDirection: "column", gap: 8 }} className="stagger">
            {/* Create form */}
            {showCreate && (
              <div className="section-card" style={{ marginBottom: 8 }}>
                <div
                  style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}
                >
                  <h3 style={{ fontSize: 14, fontWeight: 600 }}>
                    {uiText("新建客户端", "New Client", "クライアントを新規作成")}
                  </h3>
                  <button className="btn btn-ghost btn-icon-sm" onClick={closeCreate}>
                    <X size={14} />
                  </button>
                </div>
                <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                  <input
                    className="input"
                    placeholder={uiText(
                      "客户端名称（如 Claude Desktop）",
                      "Client name (e.g. Claude Desktop)",
                      "クライアント名（例: Claude Desktop）",
                    )}
                    value={newName}
                    onChange={(e) => setNewName(e.target.value)}
                  />
                  <input
                    className="input"
                    placeholder={uiText(
                      "配置文件路径（可选）",
                      "Config file path (optional)",
                      "設定ファイルパス（任意）",
                    )}
                    value={newConfigPath}
                    onChange={(e) => setNewConfigPath(e.target.value)}
                    style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
                  />
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

            {clients.map((client) => {
              const accessCount = Object.values(client.server_access).filter(Boolean).length;
              return (
                <McpClientCard
                  key={client.id}
                  client={client}
                  selected={selected?.id === client.id}
                  serverCountLabel={uiText(
                    `可访问 ${accessCount}/${servers.length} 个服务器`,
                    `${accessCount}/${servers.length} servers accessible`,
                    `${accessCount}/${servers.length} 個のサーバーにアクセス可能`,
                  )}
                  onSelect={handleSelectClient}
                  onDelete={handleDelete}
                />
              );
            })}
          </div>

          {/* Detail Panel */}
          <div style={{ overflowY: "auto" }}>
            {selected ? (
              <div className="section-card" style={{ position: "sticky", top: 0 }}>
                <div
                  style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <Monitor size={18} style={{ color: "var(--text-secondary)" }} />
                    <h3 style={{ fontSize: 15, fontWeight: 700 }}>{selected.name}</h3>
                  </div>
                  {editing ? (
                    <div style={{ display: "flex", gap: 8 }}>
                      <button className="btn btn-secondary btn-sm" onClick={stopEditing}>
                        <X size={14} />
                        {uiText("取消", "Cancel", "キャンセル")}
                      </button>
                      <button className="btn btn-primary btn-sm" onClick={handleSaveAccess}>
                        <Save size={14} />
                        {uiText("保存", "Save", "保存")}
                      </button>
                    </div>
                  ) : (
                    <button className="btn btn-secondary btn-sm" onClick={handleStartEditSelected}>
                      <Shield size={14} />
                      {uiText("管理权限", "Manage Access", "アクセス権を管理")}
                    </button>
                  )}
                </div>

                {selected.config_path && (
                  <div style={{ marginBottom: 18 }}>
                    <span className="field-label">{uiText("配置路径", "Config Path", "設定パス")}</span>
                    <div className="code-block" style={{ fontSize: 11 }}>
                      {selected.config_path}
                    </div>
                  </div>
                )}

                <div>
                  <span className="field-label">{uiText("服务器访问权限", "Server Access", "サーバーアクセス権")}</span>
                  <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    {servers.map((server) => {
                      const hasAccess = editing
                        ? (editAccess[server.id] ?? true)
                        : (selected.server_access[server.id] ?? true);
                      return (
                        <McpClientAccessRow
                          key={server.id}
                          server={server}
                          hasAccess={hasAccess}
                          editing={editing}
                          allowedLabel={uiText("允许", "Allowed", "許可")}
                          deniedLabel={uiText("拒绝", "Denied", "拒否")}
                          onToggle={handleToggleServerAccess}
                        />
                      );
                    })}
                  </div>
                </div>
              </div>
            ) : (
              <div
                className="card"
                style={{ display: "flex", alignItems: "center", justifyContent: "center", height: 200 }}
              >
                <p style={{ fontSize: 13, color: "var(--text-muted)" }}>
                  {uiText(
                    "选择一个客户端查看详情",
                    "Select a client to view details",
                    "クライアントを選択して詳細を表示",
                  )}
                </p>
              </div>
            )}
          </div>
        </div>
      )}
      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={uiText("删除客户端", "Delete Client", "クライアントを削除")}
        message={uiText(
          `确定删除客户端「${pendingDelete?.name}」？`,
          `Delete client "${pendingDelete?.name}"?`,
          `クライアント「${pendingDelete?.name}」を削除しますか？`,
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
