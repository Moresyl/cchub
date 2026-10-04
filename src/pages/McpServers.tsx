import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, X, Plug, Activity, MonitorCheck, Upload, PackagePlus, Search } from "lucide-react";
import { t, tReplace, getLocale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import McpServerCard from "../components/McpServerCard";
import EmptyState from "../components/states/EmptyState";
import ErrorState from "../components/states/ErrorState";
import LoadingState from "../components/states/LoadingState";
import { useMcpValidation, type McpWizardDraft } from "../hooks/useMcpValidation";
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
import MasterDetailLayout from "../components/layout/MasterDetailLayout";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { SimpleSelect } from "../components/ui/simple-select";
import { useConfigSave } from "./mcp-servers/useConfigSave";
import { useSyncStatus } from "./mcp-servers/useSyncStatus";
import { useConfigCopy } from "./mcp-servers/useConfigCopy";
import { usePageData } from "./mcp-servers/usePageData";

const MCP_SYNCABLE_APPS = [
  { id: "claude", label: "Claude" },
  { id: "claude-desktop", label: "Claude Desktop" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "grokbuild", label: "Grok Build" },
  { id: "opencode", label: "OpenCode" },
  { id: "hermes", label: "Hermes" },
  { id: "mcode", label: "MiniMax Code" },
] as const;

const MCP_SYNCABLE_TOOL_IDS = new Set<string>(MCP_SYNCABLE_APPS.map((app) => app.id));

export default function McpServers() {
  const zh = getLocale() === "zh";
  const {
    servers,
    setServers,
    tools,
    selected,
    setSelected,
    loading,
    loadError,
    serverAppStatus,
    setServerAppStatus,
    appStatusLoading,
    loadPageData,
  } = usePageData(zh);
  const [editing, setEditing] = useState(false);
  const [editCommand, setEditCommand] = useState("");
  const [editArgs, setEditArgs] = useState("");
  const [editEnv, setEditEnv] = useState("");
  const [editRevision, setEditRevision] = useState<string>();
  const [healthResults, setHealthResults] = useState<Record<string, HealthCheckResult>>({});
  const [checkingHealth, setCheckingHealth] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const installedTools = useMemo(
    () =>
      tools.filter(
        (tool) =>
          tool.installed && MANAGED_APPS.includes(tool.id as ManagedAppId) && MCP_SYNCABLE_TOOL_IDS.has(tool.id),
      ),
    [tools],
  );
  const [pendingDelete, setPendingDelete] = useState<McpServer | null>(null);
  const [runtimeDeps, setRuntimeDeps] = useState<RuntimeDepStatus[]>([]);
  const [showDeps, setShowDeps] = useState(false);
  const [checkingDeps, setCheckingDeps] = useState(false);
  const [wizardOpen, setWizardOpen] = useState(false);
  const installingRef = useRef(false);
  const deletingRef = useRef(false);
  const [wizardStep, setWizardStep] = useState(1);
  const [wizardInstalling, setWizardInstalling] = useState(false);
  const [wizardSyncTargets, setWizardSyncTargets] = useState<string[]>([]);
  const [wizardDraft, setWizardDraft] = useState<McpWizardDraft>({
    name: "",
    transport: "stdio",
    command: "",
    argsText: "",
    envText: "",
  });
  const [search, setSearch] = useState("");
  const [bulkApp, setBulkApp] = useState<string>(MCP_SYNCABLE_APPS[0].id);
  const i = t();
  const syncStatus = useSyncStatus(selected, zh);
  const configCopy = useConfigCopy(selected, zh);
  const { save: saveConfig, saving: configSaving, isSaving: isConfigSaving } = useConfigSave(zh);
  const wizardValidation = useMcpValidation(wizardDraft, zh);
  const wizardSyncableTools = installedTools.filter((tool) => tool.id !== "claude");
  const bulkToggleMcpAppMutation = useBulkToggleMcpAppMutation();
  const uninstallMcpServerMutation = useUninstallMcpServerMutation();
  const updateMcpServerConfigMutation = useUpdateMcpServerConfigMutation();
  const installMcpServerMutation = useInstallMcpServerMutation<McpServer>();

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
      if (deletingRef.current) return;
      deletingRef.current = true;
      try {
        await uninstallMcpServerMutation.mutateAsync({ name: server.id, revision: server.origin?.revision });
        setServers((prev) => prev.filter((s) => s.id !== server.id));
        if (selected?.id === server.id) setSelected(null);
        setPendingDelete(null);
      } catch (e) {
        console.error(e);
        showToast(
          "error",
          zh ? "移除失败，配置未被确认删除。请刷新后重试。" : "Removal failed. Refresh the configuration and retry.",
        );
      } finally {
        deletingRef.current = false;
      }
    },
    [selected, setSelected, setServers, uninstallMcpServerMutation, zh],
  );

  const startEdit = useCallback((server: McpServer) => {
    setEditRevision(server.origin?.revision);
    setEditing(true);
    setSaveSuccess(false);
    setEditCommand(server.command || "");
    setEditArgs(formatJson(server.args));
    setEditEnv(formatJson(server.env));
  }, []);

  const handleSave = useCallback(async () => {
    if (!selected) return;
    const saved = await saveConfig(
      { name: selected.id, transport: selected.transport, command: editCommand, args: editArgs, env: editEnv },
      (config) => updateMcpServerConfigMutation.mutateAsync({ ...config, revision: editRevision }),
    );
    if (saved) {
      setEditing(false);
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 3000);
      await loadPageData({ force: true });
    }
  }, [saveConfig, editArgs, editCommand, editEnv, editRevision, loadPageData, selected, updateMcpServerConfigMutation]);

  const openWizard = useCallback(() => {
    if (installingRef.current) return;
    setWizardDraft({
      name: "",
      transport: "stdio",
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
    if (installingRef.current) return;
    setWizardOpen(false);
    setWizardInstalling(false);
  }, []);

  const handleWizardInstall = useCallback(async () => {
    if (!wizardValidation.isValid || installingRef.current) return;
    installingRef.current = true;

    setWizardInstalling(true);
    try {
      const created = await installMcpServerMutation.mutateAsync({
        name: wizardDraft.name.trim(),
        transport: wizardDraft.transport,
        command: wizardDraft.command.trim(),
        args: wizardValidation.parsedArgs,
        env: wizardValidation.parsedEnv,
        targets: wizardSyncTargets,
      });
      setSelected(created);
      setWizardOpen(false);
      await loadPageData({ force: true });
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
      installingRef.current = false;
      setWizardInstalling(false);
    }
  }, [installMcpServerMutation, loadPageData, setSelected, wizardDraft, wizardSyncTargets, wizardValidation, zh]);

  const toggleToolSync = useCallback(
    async (toolId: string) => {
      const result = await syncStatus.toggle(toolId);
      if (result) {
        setServerAppStatus((prev) => ({
          ...prev,
          [result.serverId]: { ...(prev[result.serverId] ?? {}), [toolId]: result.enabled },
        }));
      }
    },
    [syncStatus, setServerAppStatus],
  );

  const handleSelectServer = useCallback(
    (server: McpServer) => {
      setSelected(server);
      setEditing(false);
      setSaveSuccess(false);
    },
    [setSelected],
  );
  const handleBulkToggle = useCallback(
    async (enabled: boolean) => {
      if (bulkToggleMcpAppMutation.isPending || appStatusLoading || servers.length === 0) return;
      if (servers.some((server) => typeof serverAppStatus[server.id]?.[bulkApp] !== "boolean")) {
        showToast(
          "error",
          zh ? "部分服务的同步状态未知，请刷新后重试。" : "Some sync states are unknown. Refresh before retrying.",
        );
        return;
      }
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
    [appStatusLoading, bulkApp, bulkToggleMcpAppMutation, i.mcp, servers, serverAppStatus, setServerAppStatus, zh],
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
    [startEdit, setSelected],
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
      if (isConfigSaving()) return;
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
  }, [closeWizard, isConfigSaving, editing, handleSave, openWizard, selected, wizardOpen]);

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
        saving={configSaving}
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
        <div className="page-action-group">
          <Button size="sm" onClick={openWizard} style={{ gap: 6 }}>
            <PackagePlus size={14} />
            {zh ? "安装向导" : "Install Wizard"}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void checkDeps()}
            disabled={checkingDeps}
            style={{ gap: 6 }}
          >
            <MonitorCheck size={14} />
            {zh ? "环境检查" : "Env Check"}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => void handleImportServers()} style={{ gap: 6 }}>
            <Upload size={14} />
            {i.mcp.importServer}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => void checkHealth()} disabled={checkingHealth}>
            <Activity size={14} />
            {checkingHealth ? i.mcp.checking : i.mcp.checkHealth}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => void loadPageData({ force: true })}>
            <RefreshCw size={14} />
            {i.mcp.refresh}
          </Button>
        </div>
      </div>

      <div className="page-filter-bar">
        <div className="page-search-field">
          <Search size={14} />
          <Input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder={i.mcp.searchPlaceholder}
            aria-label={i.mcp.searchPlaceholder}
          />
        </div>
        {availableMcpApps.length > 0 && (
          <div className="bulk-action-strip">
            <span className="bulk-action-strip-label">{i.mcp.bulkApp}</span>
            <SimpleSelect
              value={bulkApp}
              onValueChange={setBulkApp}
              disabled={appStatusLoading || bulkToggleMcpAppMutation.isPending}
              ariaLabel={i.mcp.bulkApp}
              className="w-[138px]"
              options={availableMcpApps.map((app) => ({ value: app.id, label: app.label }))}
            />
            <span className="badge badge-muted" title={zh ? "已同步数量 / 总数量" : "Synced / total"}>
              {appStatusLoading ? "..." : `${bulkEnabledCount}/${servers.length}`}
            </span>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void handleBulkToggle(true)}
              disabled={appStatusLoading || bulkToggleMcpAppMutation.isPending || servers.length === 0}
            >
              {bulkToggleMcpAppMutation.isPending ? i.mcp.bulkRunning : i.mcp.bulkEnable}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void handleBulkToggle(false)}
              disabled={appStatusLoading || bulkToggleMcpAppMutation.isPending || servers.length === 0}
            >
              {i.mcp.bulkDisable}
            </Button>
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
            <Button variant="ghost" size="icon-sm" onClick={() => setShowDeps(false)}>
              <X size={14} />
            </Button>
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
                    <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "var(--font-code)" }}>
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
        <MasterDetailLayout
          detailLabel={selected ? `${selected.name} ${i.mcp.detail}` : i.mcp.detail}
          list={
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }} className="stagger">
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
          }
          detail={
            selected ? (
              <McpServerDetailPanel
                selected={selected}
                i={i}
                zh={zh}
                copied={configCopy.copied}
                copying={configCopy.copying}
                copyConfig={configCopy.copy}
                startEdit={startEdit}
                saveSuccess={saveSuccess}
                healthResult={healthResults[selected.id]}
                getSourceBadge={getSourceBadge}
                getSourceLabel={getSourceLabel}
                installedTools={installedTools}
                toolSyncStatus={syncStatus.status}
                toolStates={syncStatus.states}
                syncingTo={syncStatus.syncingTo}
                statusLoading={syncStatus.loading}
                statusError={syncStatus.error}
                refreshStatus={syncStatus.refresh}
                toggleToolSync={toggleToolSync}
                onClose={() => setSelected(null)}
              />
            ) : undefined
          }
        />
      )}
      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={i.mcp?.remove || "移除"}
        message={pendingDelete ? tReplace(i.mcp.confirmRemove, { name: pendingDelete.name }) : ""}
        confirmText={i.mcp?.remove || "移除"}
        variant="destructive"
        busy={uninstallMcpServerMutation.isPending}
        onConfirm={() => {
          if (pendingDelete) void doDelete(pendingDelete);
        }}
        onCancel={() => {
          if (!deletingRef.current) setPendingDelete(null);
        }}
      />
    </div>
  );
}
