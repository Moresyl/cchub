import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, X, Plug, Activity, MonitorCheck, Upload, PackagePlus, Search } from "lucide-react";
import { t, tReplace, getLocale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import McpServerCard from "../components/McpServerCard";
import EmptyState from "../components/states/EmptyState";
import ErrorState from "../components/states/ErrorState";
import LoadingState from "../components/states/LoadingState";
import type { DetectedTool } from "../types/skills";
import { useMcpValidation, type McpWizardDraft } from "../hooks/useMcpValidation";
import { fetchMcpServersPageData, queryKeys } from "../hooks/queries";
import {
  useInstallMcpServerMutation,
  useBulkToggleMcpAppMutation,
  useUninstallMcpServerMutation,
  useUpdateMcpServerConfigMutation,
} from "../hooks/mutations";
import { MANAGED_APPS, type ManagedAppId } from "../lib/appPreferences";
import {
  formatJson,
  type HealthCheckResult,
  type McpServer,
  type RuntimeDepStatus,
  type WizardPreset,
} from "./mcp-servers/helpers";
import McpServerEditView from "./mcp-servers/EditView";
import McpServerWizardView from "./mcp-servers/WizardView";
import McpServerDetailPanel from "./mcp-servers/DetailPanel";

const MCP_SYNCABLE_APPS = [
  { id: "claude", label: "Claude" },
  { id: "claude-desktop", label: "Claude Desktop" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "grokbuild", label: "Grok Build" },
  { id: "opencode", label: "OpenCode" },
  { id: "hermes", label: "Hermes" },
] as const;

export default function McpServers() {
  const queryClient = useQueryClient();
  const cachedMcpServersPageData = queryClient.getQueryData<Awaited<ReturnType<typeof fetchMcpServersPageData>>>(
    queryKeys.mcpServersPage,
  );
  const [servers, setServers] = useState<McpServer[]>(cachedMcpServersPageData?.servers ?? []);
  const [loading, setLoading] = useState(!cachedMcpServersPageData);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState<McpServer | null>(null);
  const [editing, setEditing] = useState(false);
  const [editCommand, setEditCommand] = useState("");
  const [editArgs, setEditArgs] = useState("");
  const [editEnv, setEditEnv] = useState("");
  const [copied, setCopied] = useState(false);
  const [healthResults, setHealthResults] = useState<Record<string, HealthCheckResult>>({});
  const [checkingHealth, setCheckingHealth] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [syncingTo, setSyncingTo] = useState<string | null>(null);
  const [installedTools, setInstalledTools] = useState<DetectedTool[]>(
    cachedMcpServersPageData?.tools.filter(
      (tool) => tool.installed && MANAGED_APPS.includes(tool.id as ManagedAppId) && tool.id !== "openclaw",
    ) ?? [],
  );
  const [toolSyncStatus, setToolSyncStatus] = useState<Record<string, boolean>>({});
  const [pendingDelete, setPendingDelete] = useState<McpServer | null>(null);
  const [runtimeDeps, setRuntimeDeps] = useState<RuntimeDepStatus[]>([]);
  const [showDeps, setShowDeps] = useState(false);
  const [checkingDeps, setCheckingDeps] = useState(false);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardStep, setWizardStep] = useState(1);
  const [wizardInstalling, setWizardInstalling] = useState(false);
  const [wizardSyncTargets, setWizardSyncTargets] = useState<string[]>([]);
  const [wizardDraft, setWizardDraft] = useState<McpWizardDraft>({
    name: "",
    command: "",
    argsText: "",
    envText: "",
  });
  const [search, setSearch] = useState("");
  const [bulkApp, setBulkApp] = useState<string>(MCP_SYNCABLE_APPS[0].id);
  const [serverAppStatus, setServerAppStatus] = useState<Record<string, Record<string, boolean>>>({});
  const [appStatusLoading, setAppStatusLoading] = useState(false);
  const i = t();
  const zh = getLocale() === "zh";
  const wizardValidation = useMcpValidation(wizardDraft);
  const wizardSyncableTools = installedTools.filter((tool) => tool.id !== "claude");
  const bulkToggleMcpAppMutation = useBulkToggleMcpAppMutation();
  const uninstallMcpServerMutation = useUninstallMcpServerMutation();
  const updateMcpServerConfigMutation = useUpdateMcpServerConfigMutation();
  const installMcpServerMutation = useInstallMcpServerMutation<McpServer>();

  const loadPageData = useCallback(
    async (options: { force?: boolean } = {}) => {
      const { force = false } = options;
      if (!queryClient.getQueryData(queryKeys.mcpServersPage)) {
        setLoading(true);
      }
      setLoadError(null);
      try {
        const data = await queryClient.fetchQuery({
          queryKey: queryKeys.mcpServersPage,
          queryFn: fetchMcpServersPageData,
          staleTime: force ? 0 : 30_000,
        });
        setServers(data.servers);
        setInstalledTools(
          data.tools.filter(
            (tool) => tool.installed && MANAGED_APPS.includes(tool.id as ManagedAppId) && tool.id !== "openclaw",
          ),
        );
        setSelected((current) =>
          current ? (data.servers.find((server) => server.id === current.id) ?? current) : current,
        );
        setAppStatusLoading(true);
        const statusResults = await Promise.allSettled(
          data.servers.map(
            async (server) =>
              [
                server.id,
                await invoke<Record<string, boolean>>("check_mcp_server_in_tools", { serverName: server.name }),
              ] as const,
          ),
        );
        const nextStatus: Record<string, Record<string, boolean>> = {};
        for (const result of statusResults) {
          if (result.status === "fulfilled") nextStatus[result.value[0]] = result.value[1];
        }
        setServerAppStatus(nextStatus);
      } catch (e) {
        setLoadError(String(e));
      } finally {
        setAppStatusLoading(false);
        setLoading(false);
      }
    },
    [queryClient],
  );

  const checkDeps = useCallback(async () => {
    setCheckingDeps(true);
    setShowDeps(true);
    try {
      setRuntimeDeps(await invoke<RuntimeDepStatus[]>("check_runtime_dependencies"));
    } catch (e) {
      console.error(e);
    } finally {
      setCheckingDeps(false);
    }
  }, []);

  const checkHealth = useCallback(async () => {
    setCheckingHealth(true);
    try {
      const results = await invoke<HealthCheckResult[]>("check_all_mcp_health");
      const map: Record<string, HealthCheckResult> = {};
      for (const r of results) {
        map[r.server_id] = r;
      }
      setHealthResults(map);
    } catch (e) {
      console.error(e);
    } finally {
      setCheckingHealth(false);
    }
  }, []);

  const handleDelete = useCallback((server: McpServer) => {
    setPendingDelete(server);
  }, []);

  const doDelete = useCallback(
    async (server: McpServer) => {
      try {
        await uninstallMcpServerMutation.mutateAsync({ name: server.name });
        setServers((prev) => prev.filter((s) => s.id !== server.id));
        if (selected?.id === server.id) setSelected(null);
      } catch (e) {
        console.error(e);
      }
    },
    [selected, uninstallMcpServerMutation],
  );

  const startEdit = useCallback((server: McpServer) => {
    setEditing(true);
    setSaveSuccess(false);
    setEditCommand(server.command || "");
    setEditArgs(formatJson(server.args));
    setEditEnv(formatJson(server.env));
  }, []);

  const handleSave = useCallback(async () => {
    if (!selected) return;
    try {
      const args = JSON.parse(editArgs);
      const env = JSON.parse(editEnv);
      await updateMcpServerConfigMutation.mutateAsync({ name: selected.name, command: editCommand, args, env });
      setEditing(false);
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 3000);
      await loadPageData({ force: true });
    } catch (e) {
      console.error(e);
      showToast("error", zh ? "JSON 格式错误，请检查参数和环境变量" : "Invalid JSON format");
    }
  }, [editArgs, editCommand, editEnv, loadPageData, selected, updateMcpServerConfigMutation, zh]);

  const openWizard = useCallback(() => {
    setWizardDraft({
      name: "",
      command: "",
      argsText: "",
      envText: "",
    });
    setWizardSyncTargets([]);
    setWizardStep(1);
    setWizardOpen(true);
  }, []);

  const applyWizardPreset = useCallback((preset: WizardPreset) => {
    setWizardDraft((current) => ({
      ...current,
      command: preset.command,
      argsText: preset.args.join("\n"),
    }));
  }, []);

  const closeWizard = useCallback(() => {
    setWizardOpen(false);
    setWizardInstalling(false);
  }, []);

  const handleWizardInstall = useCallback(async () => {
    if (!wizardValidation.isValid || wizardInstalling) return;

    setWizardInstalling(true);
    try {
      const created = await installMcpServerMutation.mutateAsync({
        name: wizardDraft.name.trim(),
        command: wizardDraft.command.trim(),
        args: wizardValidation.parsedArgs,
        env: wizardValidation.parsedEnv,
      });

      for (const toolId of wizardSyncTargets) {
        await invoke("sync_mcp_server_to_tool", {
          serverName: created.name,
          targetTool: toolId,
        });
      }

      const health = await invoke<HealthCheckResult>("check_mcp_server_health", { name: created.name });
      setHealthResults((current) => ({ ...current, [health.server_id]: health }));
      await loadPageData({ force: true });
      setSelected(created);
      closeWizard();
      showToast(
        "success",
        zh
          ? `MCP 已安装${wizardSyncTargets.length > 0 ? `，并同步到 ${wizardSyncTargets.length} 个工具` : ""}`
          : `MCP installed${wizardSyncTargets.length > 0 ? ` and synced to ${wizardSyncTargets.length} tool(s)` : ""}`,
      );
    } catch (error) {
      console.error(error);
      showToast("error", String(error));
    } finally {
      setWizardInstalling(false);
    }
  }, [
    closeWizard,
    installMcpServerMutation,
    loadPageData,
    wizardDraft,
    wizardInstalling,
    wizardSyncTargets,
    wizardValidation,
    zh,
  ]);

  const toggleToolSync = useCallback(
    async (toolId: string) => {
      if (!selected) return;
      const isSynced = toolSyncStatus[toolId];
      setSyncingTo(toolId);
      try {
        if (isSynced) {
          await invoke("unsync_mcp_server_from_tool", { serverName: selected.name, targetTool: toolId });
        } else {
          await invoke("sync_mcp_server_to_tool", { serverName: selected.name, targetTool: toolId });
        }
        const nextSynced = !isSynced;
        setToolSyncStatus((prev) => ({ ...prev, [toolId]: nextSynced }));
        setServerAppStatus((prev) => ({
          ...prev,
          [selected.id]: { ...(prev[selected.id] ?? {}), [toolId]: nextSynced },
        }));
      } catch (e) {
        console.error(e);
      } finally {
        setSyncingTo(null);
      }
    },
    [selected, toolSyncStatus],
  );

  const copyConfig = useCallback(() => {
    if (!selected) return;
    const config = {
      command: selected.command,
      args: (() => {
        try {
          return JSON.parse(selected.args);
        } catch {
          return selected.args;
        }
      })(),
      env: (() => {
        try {
          return JSON.parse(selected.env);
        } catch {
          return selected.env;
        }
      })(),
    };
    navigator.clipboard.writeText(JSON.stringify(config, null, 2));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [selected]);
  const handleSelectServer = useCallback((server: McpServer) => {
    setSelected(server);
    setEditing(false);
    setSaveSuccess(false);
    void invoke<Record<string, boolean>>("check_mcp_server_in_tools", { serverName: server.name })
      .then(setToolSyncStatus)
      .catch(() => setToolSyncStatus({}));
  }, []);
  const handleBulkToggle = useCallback(
    async (enabled: boolean) => {
      if (bulkToggleMcpAppMutation.isPending || appStatusLoading || servers.length === 0) return;
      const serverIds = servers
        .filter((server) => Boolean(serverAppStatus[server.id]?.[bulkApp]) !== enabled)
        .map((server) => server.id);
      if (serverIds.length === 0) {
        showToast("success", zh ? "所有服务已经是目标状态" : "All servers already have the requested state");
        return;
      }
      const result = await bulkToggleMcpAppMutation.mutateAsync({ serverIds, app: bulkApp, enabled });
      setServerAppStatus((current) => {
        const next = { ...current };
        for (const serverId of result.succeeded) {
          next[serverId] = { ...(next[serverId] ?? {}), [bulkApp]: enabled };
        }
        return next;
      });
      if (result.failed.length > 0) {
        showToast("error", tReplace(i.mcp.bulkFailed, { count: result.failed.length }));
      } else {
        showToast("success", enabled ? i.mcp.bulkEnable : i.mcp.bulkDisable);
      }
    },
    [appStatusLoading, bulkApp, bulkToggleMcpAppMutation, i.mcp, servers, serverAppStatus, zh],
  );
  const filteredServers = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return servers;
    return servers.filter((server) => {
      // Keep this allow-list free of env values and credentials.
      const searchable = [
        server.id,
        server.name,
        server.command,
        server.args,
        server.transport,
        server.source,
        server.package_name,
        server.version,
        server.config_path,
      ];
      return searchable.some((value) => value?.toLowerCase().includes(query));
    });
  }, [search, servers]);
  const bulkEnabledCount = servers.reduce(
    (count, server) => count + (serverAppStatus[server.id]?.[bulkApp] ? 1 : 0),
    0,
  );
  const availableMcpApps = useMemo(() => {
    const installedIds = new Set(installedTools.map((tool) => tool.id));
    return MCP_SYNCABLE_APPS.filter((app) => {
      if (app.id === "claude-desktop") {
        return Object.values(serverAppStatus).some((status) => status[app.id]);
      }
      return installedIds.has(app.id);
    });
  }, [installedTools, serverAppStatus]);

  useEffect(() => {
    if (availableMcpApps.length > 0 && !availableMcpApps.some((app) => app.id === bulkApp)) {
      setBulkApp(availableMcpApps[0].id);
    }
  }, [availableMcpApps, bulkApp]);

  const handleEditServer = useCallback(
    (server: McpServer) => {
      setSelected(server);
      startEdit(server);
    },
    [startEdit],
  );

  const handleDeleteServer = useCallback(
    (server: McpServer) => {
      handleDelete(server);
    },
    [handleDelete],
  );

  const handleImportServers = useCallback(async () => {
    try {
      const count = await invoke<number>("import_mcp_servers_from_file");
      showToast("success", `${i.mcp.importSuccess} (${count})`);
      await loadPageData({ force: true });
    } catch (e) {
      const msg = String(e);
      if (msg !== "Cancelled") showToast("error", msg);
    }
  }, [i.mcp.importSuccess, loadPageData]);

  useEffect(() => {
    void loadPageData();
  }, [loadPageData]);

  useEffect(() => {
    const handleSaveShortcut = () => {
      if (editing && selected) {
        void handleSave();
      }
    };
    const handleNewShortcut = () => {
      if (!editing && !wizardOpen) {
        openWizard();
      }
    };
    const handleEscapeShortcut = () => {
      if (wizardOpen) {
        closeWizard();
        return;
      }
      if (editing) {
        setEditing(false);
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
  }, [closeWizard, editing, handleSave, openWizard, selected, wizardOpen]);

  function getSourceLabel(source: string) {
    switch (source) {
      case "official-plugin":
        return i.mcp.officialPlugin;
      case "community-plugin":
        return i.mcp.communityPlugin;
      case "claude-desktop":
        return i.mcp.claudeDesktop;
      case "cursor":
        return i.mcp.cursor;
      default:
        return i.mcp.local;
    }
  }

  function getSourceBadge(source: string) {
    switch (source) {
      case "official-plugin":
        return "badge-accent";
      case "community-plugin":
        return "badge-success";
      case "claude-desktop":
        return "badge-warning";
      case "cursor":
        return "badge-accent";
      default:
        return "badge-muted";
    }
  }

  if (loading) {
    return <LoadingState label={i.mcp.loading} />;
  }

  // --- 编辑视图 ---
  if (editing && selected) {
    return (
      <McpServerEditView
        selected={selected}
        i={i}
        zh={zh}
        editCommand={editCommand}
        setEditCommand={setEditCommand}
        editArgs={editArgs}
        setEditArgs={setEditArgs}
        editEnv={editEnv}
        setEditEnv={setEditEnv}
        setEditing={setEditing}
        handleSave={handleSave}
      />
    );
  }

  // --- 列表视图 ---
  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.mcp.title}</h2>
          <p className="page-subtitle">{tReplace(i.mcp.serverCount, { count: servers.length })}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-primary btn-sm" onClick={openWizard} style={{ gap: 6 }}>
            <PackagePlus size={14} />
            {zh ? "安装向导" : "Install Wizard"}
          </button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => void checkDeps()}
            disabled={checkingDeps}
            style={{ gap: 6 }}
          >
            <MonitorCheck size={14} />
            {zh ? "环境检查" : "Env Check"}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => void handleImportServers()} style={{ gap: 6 }}>
            <Upload size={14} />
            {i.mcp.importServer}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => void checkHealth()} disabled={checkingHealth}>
            <Activity size={14} />
            {checkingHealth ? i.mcp.checking : i.mcp.checkHealth}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => void loadPageData({ force: true })}>
            <RefreshCw size={14} />
            {i.mcp.refresh}
          </button>
        </div>
      </div>

      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center", marginBottom: 12 }}>
        <div style={{ position: "relative", flex: "1 1 280px", minWidth: 220 }}>
          <Search
            size={14}
            style={{
              position: "absolute",
              left: 10,
              top: "50%",
              transform: "translateY(-50%)",
              color: "var(--text-muted)",
            }}
          />
          <input
            className="input"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder={i.mcp.searchPlaceholder}
            aria-label={i.mcp.searchPlaceholder}
            style={{ width: "100%", paddingLeft: 32 }}
          />
        </div>
        {availableMcpApps.length > 0 && (
          <div
            className="section-card"
            style={{ display: "flex", alignItems: "center", gap: 8, padding: "6px 8px", flex: "0 1 auto" }}
          >
            <span style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "nowrap" }}>{i.mcp.bulkApp}</span>
            <select
              className="input"
              value={bulkApp}
              onChange={(event) => setBulkApp(event.target.value)}
              disabled={appStatusLoading || bulkToggleMcpAppMutation.isPending}
              style={{ minWidth: 130, width: "auto", padding: "5px 8px" }}
            >
              {availableMcpApps.map((app) => (
                <option key={app.id} value={app.id}>
                  {app.label}
                </option>
              ))}
            </select>
            <span className="badge badge-muted" title={zh ? "已同步数量 / 总数量" : "Synced / total"}>
              {appStatusLoading ? "..." : `${bulkEnabledCount}/${servers.length}`}
            </span>
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => void handleBulkToggle(true)}
              disabled={appStatusLoading || bulkToggleMcpAppMutation.isPending || servers.length === 0}
            >
              {bulkToggleMcpAppMutation.isPending ? i.mcp.bulkRunning : i.mcp.bulkEnable}
            </button>
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => void handleBulkToggle(false)}
              disabled={appStatusLoading || bulkToggleMcpAppMutation.isPending || servers.length === 0}
            >
              {i.mcp.bulkDisable}
            </button>
          </div>
        )}
      </div>
      {search.trim() && (
        <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 8 }}>
          {zh
            ? `显示 ${filteredServers.length} / ${servers.length} 个服务`
            : `Showing ${filteredServers.length} of ${servers.length} servers`}
        </div>
      )}

      {wizardOpen && (
        <McpServerWizardView
          i={i}
          zh={zh}
          wizardStep={wizardStep}
          setWizardStep={setWizardStep}
          wizardDraft={wizardDraft}
          setWizardDraft={setWizardDraft}
          wizardSyncableTools={wizardSyncableTools}
          wizardSyncTargets={wizardSyncTargets}
          setWizardSyncTargets={setWizardSyncTargets}
          wizardValidation={wizardValidation}
          wizardInstalling={wizardInstalling}
          applyWizardPreset={applyWizardPreset}
          closeWizard={closeWizard}
          handleWizardInstall={handleWizardInstall}
        />
      )}

      {/* Runtime Dependencies Panel */}
      {showDeps && (
        <div className="section-card" style={{ marginBottom: 16 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <MonitorCheck size={15} style={{ color: "var(--text-secondary)" }} />
              <span style={{ fontSize: 13, fontWeight: 600 }}>{zh ? "运行环境检查" : "Runtime Environment"}</span>
            </div>
            <button className="btn btn-ghost btn-icon-sm" onClick={() => setShowDeps(false)}>
              <X size={14} />
            </button>
          </div>
          {checkingDeps ? (
            <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 0" }}>
              <div className="spinner" style={{ width: 14, height: 14 }} />
              <span style={{ fontSize: 12, color: "var(--text-muted)" }}>{zh ? "检测中..." : "Checking..."}</span>
            </div>
          ) : (
            <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
              {runtimeDeps.map((dep) => (
                <div
                  key={dep.name}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    padding: "6px 12px",
                    borderRadius: 6,
                    background: dep.installed ? "var(--success-subtle)" : "var(--bg-tertiary)",
                    border: `1px solid ${dep.installed ? "var(--success)" : "var(--border-default)"}`,
                  }}
                >
                  <span className={`dot ${dep.installed ? "dot-active" : "dot-disabled"}`} />
                  <span style={{ fontSize: 12, fontWeight: 500 }}>{dep.display_name}</span>
                  {dep.version && (
                    <span
                      style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}
                    >
                      {dep.version}
                    </span>
                  )}
                  {!dep.installed && (
                    <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{zh ? "未安装" : "Not installed"}</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {loadError ? (
        <ErrorState
          title={zh ? "加载 MCP 服务器失败" : "Failed to load MCP servers"}
          message={loadError}
          retryLabel={i.common.refresh}
          onRetry={() => {
            void loadPageData({ force: true });
          }}
        />
      ) : servers.length === 0 ? (
        <EmptyState
          title={i.mcp.noServers}
          description={i.mcp.noServersTip}
          icon={<Plug size={28} style={{ color: "var(--text-muted)" }} />}
        />
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "1.2fr 1fr", gap: 24, flex: 1, minHeight: 0 }}>
          {/* Server List */}
          <div style={{ overflowY: "auto", display: "flex", flexDirection: "column", gap: 8 }} className="stagger">
            {filteredServers.map((server) => (
              <McpServerCard
                key={server.id}
                server={server}
                selected={selected?.id === server.id}
                sourceBadge={getSourceBadge(server.source)}
                sourceLabel={getSourceLabel(server.source)}
                healthStatus={healthResults[server.id]?.status ?? null}
                healthTitle={
                  healthResults[server.id]
                    ? healthResults[server.id]?.status === "healthy"
                      ? i.mcp.healthy
                      : healthResults[server.id]?.status === "unhealthy"
                        ? i.mcp.unhealthy
                        : i.mcp.unknown
                    : null
                }
                editTitle={i.mcp.edit}
                deleteTitle={i.mcp.remove}
                onSelect={handleSelectServer}
                onEdit={handleEditServer}
                onDelete={handleDeleteServer}
              />
            ))}
          </div>

          {/* Detail Panel */}
          <div style={{ overflowY: "auto" }}>
            {selected ? (
              <McpServerDetailPanel
                selected={selected}
                i={i}
                zh={zh}
                copied={copied}
                copyConfig={copyConfig}
                startEdit={startEdit}
                saveSuccess={saveSuccess}
                healthResult={healthResults[selected.id]}
                getSourceBadge={getSourceBadge}
                getSourceLabel={getSourceLabel}
                installedTools={installedTools}
                toolSyncStatus={toolSyncStatus}
                syncingTo={syncingTo}
                toggleToolSync={toggleToolSync}
              />
            ) : (
              <div
                className="card"
                style={{ display: "flex", alignItems: "center", justifyContent: "center", height: 200 }}
              >
                <p style={{ fontSize: 13, color: "var(--text-muted)" }}>{i.mcp.selectServer}</p>
              </div>
            )}
          </div>
        </div>
      )}
      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={i.mcp?.remove || "移除"}
        message={pendingDelete ? tReplace(i.mcp.confirmRemove, { name: pendingDelete.name }) : ""}
        confirmText={i.mcp?.remove || "移除"}
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
