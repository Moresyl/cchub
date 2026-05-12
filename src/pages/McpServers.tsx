import { useQueryClient } from "@tanstack/react-query";
import { lazy, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw,
  Edit3,
  X,
  Save,
  Plug,
  Copy,
  Check,
  Activity,
  FileText,
  Share2,
  Wand2,
  MonitorCheck,
  Upload,
  PackagePlus,
  ArrowLeft,
  ArrowRight,
} from "lucide-react";
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
  useToggleMcpServerMutation,
  useUninstallMcpServerMutation,
  useUpdateMcpServerConfigMutation,
} from "../hooks/mutations";
import { MANAGED_APPS, type ManagedAppId } from "../lib/appPreferences";
const CodeEditor = lazy(() => import("../components/CodeEditor"));

import {
  WIZARD_PRESETS,
  formatJson,
  type HealthCheckResult,
  type McpServer,
  type RuntimeDepStatus,
  type WizardPreset,
} from "./mcp-servers/helpers";

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
  const i = t();
  const zh = getLocale() === "zh";
  const wizardValidation = useMcpValidation(wizardDraft);
  const wizardSyncableTools = installedTools.filter((tool) => tool.id !== "claude");
  const toggleMcpServerMutation = useToggleMcpServerMutation();
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
      } catch (e) {
        setLoadError(String(e));
      } finally {
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

  const handleToggle = useCallback(
    async (server: McpServer) => {
      const newEnabled = server.status === "disabled";
      try {
        await toggleMcpServerMutation.mutateAsync({ id: server.id, enabled: newEnabled });
        const newStatus = newEnabled ? "active" : "disabled";
        setServers((prev) => prev.map((s) => (s.id === server.id ? { ...s, status: newStatus } : s)));
        if (selected?.id === server.id) setSelected({ ...server, status: newStatus });
      } catch (e) {
        console.error(e);
      }
    },
    [selected, toggleMcpServerMutation],
  );

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
        setToolSyncStatus((prev) => ({ ...prev, [toolId]: !isSynced }));
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
    invoke<Record<string, boolean>>("check_mcp_server_in_tools", { serverName: server.name })
      .then(setToolSyncStatus)
      .catch(() => setToolSyncStatus({}));
  }, []);

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
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <div className="page-header">
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <button className="btn btn-ghost btn-icon-sm" onClick={() => setEditing(false)} title={i.mcp.cancel}>
              <X size={18} />
            </button>
            <div>
              <h2 className="page-title">{selected.name}</h2>
              <p className="page-subtitle">{zh ? "编辑 MCP 服务器配置" : "Edit MCP server configuration"}</p>
            </div>
          </div>
        </div>

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            display: "flex",
            flexDirection: "column",
            gap: 20,
            paddingBottom: 20,
          }}
        >
          <div>
            <span className="field-label">{i.mcp.command}</span>
            <input
              className="input"
              style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
              value={editCommand}
              onChange={(e) => setEditCommand(e.target.value)}
              placeholder="npx, node, python..."
            />
          </div>

          <div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
              <span className="field-label" style={{ marginBottom: 0 }}>
                {i.mcp.arguments}
              </span>
              <button
                className="btn btn-ghost btn-icon-sm"
                title="Format"
                onClick={() => {
                  try {
                    setEditArgs(JSON.stringify(JSON.parse(editArgs), null, 2));
                  } catch (error) {
                    showToast("error", String(error));
                  }
                }}
              >
                <Wand2 size={12} />
              </button>
            </div>
            <CodeEditor value={editArgs} onChange={setEditArgs} language="json" minHeight={160} />
          </div>

          <div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
              <span className="field-label" style={{ marginBottom: 0 }}>
                {i.mcp.environment}
              </span>
              <button
                className="btn btn-ghost btn-icon-sm"
                title="Format"
                onClick={() => {
                  try {
                    setEditEnv(JSON.stringify(JSON.parse(editEnv), null, 2));
                  } catch (error) {
                    showToast("error", String(error));
                  }
                }}
              >
                <Wand2 size={12} />
              </button>
            </div>
            <CodeEditor value={editEnv} onChange={setEditEnv} language="json" minHeight={160} />
          </div>
        </div>

        <div className="sticky-footer" style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={() => setEditing(false)}>
            {i.mcp.cancel}
          </button>
          <button className="btn btn-primary btn-sm" onClick={handleSave} style={{ gap: 6 }}>
            <Save size={14} />
            {i.mcp.save}
          </button>
        </div>
      </div>
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

      {wizardOpen && (
        <div className="section-card" style={{ marginBottom: 16 }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: 12,
              marginBottom: 16,
              flexWrap: "wrap",
            }}
          >
            <div>
              <div style={{ fontSize: 15, fontWeight: 700 }}>{zh ? "MCP 安装向导" : "MCP Install Wizard"}</div>
              <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
                {zh
                  ? "按步骤填写命令、参数和环境变量，安装后会自动做一次健康检查。"
                  : "Fill in command, arguments, and environment step by step. A health check will run automatically after install."}
              </div>
            </div>
            <button className="btn btn-ghost btn-icon-sm" onClick={closeWizard}>
              <X size={14} />
            </button>
          </div>

          <div style={{ display: "flex", gap: 8, marginBottom: 16, flexWrap: "wrap" }}>
            {[zh ? "1. 基本配置" : "1. Basics", zh ? "2. 同步目标" : "2. Sync", zh ? "3. 复核安装" : "3. Review"].map(
              (label, index) => (
                <div
                  key={label}
                  className={`badge ${wizardStep === index + 1 ? "badge-accent" : "badge-muted"}`}
                  style={{ padding: "6px 10px" }}
                >
                  {label}
                </div>
              ),
            )}
          </div>

          {wizardStep === 1 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
              <div>
                <div className="field-label" style={{ marginBottom: 8 }}>
                  {zh ? "常用模板" : "Quick Templates"}
                </div>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                  {WIZARD_PRESETS.map((preset) => (
                    <button
                      key={preset.id}
                      className="btn btn-secondary btn-sm"
                      onClick={() => applyWizardPreset(preset)}
                    >
                      {zh ? preset.labelZh : preset.labelEn}
                    </button>
                  ))}
                </div>
              </div>

              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 14 }}>
                <div>
                  <label className="field-label">{zh ? "服务名称" : "Server Name"}</label>
                  <input
                    className="input"
                    value={wizardDraft.name}
                    onChange={(event) => setWizardDraft((current) => ({ ...current, name: event.target.value }))}
                    placeholder={zh ? "例如 filesystem" : "e.g. filesystem"}
                  />
                </div>
                <div>
                  <label className="field-label">{zh ? "命令" : "Command"}</label>
                  <input
                    className="input"
                    value={wizardDraft.command}
                    onChange={(event) => setWizardDraft((current) => ({ ...current, command: event.target.value }))}
                    placeholder="npx / uvx / docker / node"
                  />
                </div>
              </div>

              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 14 }}>
                <div>
                  <label className="field-label">{zh ? "参数" : "Arguments"}</label>
                  <textarea
                    className="input"
                    value={wizardDraft.argsText}
                    onChange={(event) => setWizardDraft((current) => ({ ...current, argsText: event.target.value }))}
                    placeholder={
                      zh ? "每行一个参数，或直接粘贴 JSON 数组" : "One argument per line, or paste a JSON array"
                    }
                    style={{
                      minHeight: 118,
                      resize: "vertical",
                      fontFamily: "'JetBrains Mono', monospace",
                      fontSize: 12,
                      paddingTop: 10,
                    }}
                  />
                </div>
                <div>
                  <label className="field-label">{zh ? "环境变量" : "Environment"}</label>
                  <textarea
                    className="input"
                    value={wizardDraft.envText}
                    onChange={(event) => setWizardDraft((current) => ({ ...current, envText: event.target.value }))}
                    placeholder={
                      zh ? "每行 KEY=value，或直接粘贴 JSON 对象" : "Use KEY=value per line, or paste a JSON object"
                    }
                    style={{
                      minHeight: 118,
                      resize: "vertical",
                      fontFamily: "'JetBrains Mono', monospace",
                      fontSize: 12,
                      paddingTop: 10,
                    }}
                  />
                </div>
              </div>
            </div>
          )}

          {wizardStep === 2 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              <div className="card" style={{ padding: 12, fontSize: 12, color: "var(--text-muted)" }}>
                {zh
                  ? "安装会默认写入 Claude 配置。下面可以额外勾选要同步到的其他工具。"
                  : "Install always writes to Claude first. Optionally sync the same server into other tools below."}
              </div>

              <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, opacity: 0.8 }}>
                  <input type="checkbox" checked readOnly />
                  Claude
                  <span style={{ color: "var(--text-muted)" }}>{zh ? "默认" : "default"}</span>
                </label>

                {wizardSyncableTools.length === 0 ? (
                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                    {zh ? "当前没有可同步的其他已安装工具。" : "No additional installed tools are available for sync."}
                  </div>
                ) : (
                  wizardSyncableTools.map((tool) => {
                    const checked = wizardSyncTargets.includes(tool.id);
                    return (
                      <label key={tool.id} style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={() =>
                            setWizardSyncTargets((current) =>
                              checked ? current.filter((item) => item !== tool.id) : [...current, tool.id],
                            )
                          }
                        />
                        {tool.name}
                      </label>
                    );
                  })
                )}
              </div>
            </div>
          )}

          {wizardStep === 3 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              {(wizardValidation.errors.length > 0 || wizardValidation.warnings.length > 0) && (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {wizardValidation.errors.map((message) => (
                    <div
                      key={`wizard-error:${message}`}
                      className="card"
                      style={{
                        padding: "10px 12px",
                        borderColor: "var(--danger)",
                        background: "color-mix(in srgb, var(--danger) 8%, var(--bg-card))",
                        fontSize: 12,
                      }}
                    >
                      {zh ? `错误：${message}` : `Error: ${message}`}
                    </div>
                  ))}
                  {wizardValidation.warnings.map((message) => (
                    <div
                      key={`wizard-warning:${message}`}
                      className="card"
                      style={{
                        padding: "10px 12px",
                        borderColor: "var(--warning)",
                        background: "color-mix(in srgb, var(--warning) 10%, var(--bg-card))",
                        fontSize: 12,
                      }}
                    >
                      {zh ? `提示：${message}` : `Warning: ${message}`}
                    </div>
                  ))}
                </div>
              )}

              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 14 }}>
                <div className="card" style={{ padding: 12 }}>
                  <div className="field-label">{zh ? "命令预览" : "Command Preview"}</div>
                  <div className="code-block" style={{ fontSize: 12 }}>
                    {[wizardDraft.command.trim(), ...wizardValidation.parsedArgs].filter(Boolean).join(" ") ||
                      i.common.na}
                  </div>
                </div>
                <div className="card" style={{ padding: 12 }}>
                  <div className="field-label">{zh ? "同步到" : "Sync Targets"}</div>
                  <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                    {wizardSyncTargets.length > 0 ? `Claude, ${wizardSyncTargets.join(", ")}` : "Claude"}
                  </div>
                </div>
              </div>

              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 14 }}>
                <div>
                  <div className="field-label">{zh ? "参数解析结果" : "Parsed Arguments"}</div>
                  <CodeEditor
                    value={JSON.stringify(wizardValidation.parsedArgs, null, 2)}
                    language="json"
                    readOnly
                    minHeight={100}
                    maxHeight={180}
                  />
                </div>
                <div>
                  <div className="field-label">{zh ? "环境变量解析结果" : "Parsed Environment"}</div>
                  <CodeEditor
                    value={JSON.stringify(wizardValidation.parsedEnv, null, 2)}
                    language="json"
                    readOnly
                    minHeight={100}
                    maxHeight={180}
                  />
                </div>
              </div>
            </div>
          )}

          <div style={{ display: "flex", justifyContent: "space-between", marginTop: 18, gap: 8, flexWrap: "wrap" }}>
            <div>
              {wizardStep > 1 && (
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={() => setWizardStep((current) => Math.max(1, current - 1))}
                  style={{ gap: 6 }}
                >
                  <ArrowLeft size={14} />
                  {zh ? "上一步" : "Back"}
                </button>
              )}
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <button className="btn btn-secondary btn-sm" onClick={closeWizard}>
                {i.common.cancel}
              </button>
              {wizardStep < 3 ? (
                <button
                  className="btn btn-primary btn-sm"
                  onClick={() => setWizardStep((current) => Math.min(3, current + 1))}
                  disabled={wizardStep === 1 && !wizardValidation.isValid}
                  style={{ gap: 6 }}
                >
                  {zh ? "下一步" : "Next"}
                  <ArrowRight size={14} />
                </button>
              ) : (
                <button
                  className="btn btn-primary btn-sm"
                  onClick={() => void handleWizardInstall()}
                  disabled={!wizardValidation.isValid || wizardInstalling}
                  style={{ gap: 6 }}
                >
                  {wizardInstalling ? (
                    <div className="spinner" style={{ width: 12, height: 12 }} />
                  ) : (
                    <PackagePlus size={14} />
                  )}
                  {wizardInstalling ? (zh ? "安装中..." : "Installing...") : zh ? "安装并验证" : "Install & Verify"}
                </button>
              )}
            </div>
          </div>
        </div>
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
            {servers.map((server) => (
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
                toggleTitle={server.status === "disabled" ? i.mcp.enable : i.mcp.disable}
                editTitle={i.mcp.edit}
                deleteTitle={i.mcp.remove}
                onSelect={handleSelectServer}
                onToggle={handleToggle}
                onEdit={handleEditServer}
                onDelete={handleDeleteServer}
              />
            ))}
          </div>

          {/* Detail Panel */}
          <div style={{ overflowY: "auto" }}>
            {selected ? (
              <div className="section-card" style={{ position: "sticky", top: 0 }}>
                {/* Panel Header */}
                <div
                  style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <span
                      className={`dot ${selected.status === "active" ? "dot-active" : selected.status === "error" ? "dot-error" : "dot-disabled"}`}
                    />
                    <h3 style={{ fontSize: 15, fontWeight: 700 }}>{selected.name}</h3>
                    {selected.version && (
                      <span style={{ fontSize: 11, color: "var(--text-muted)" }}>v{selected.version}</span>
                    )}
                  </div>
                  <div style={{ display: "flex", gap: 6 }}>
                    <button className="btn btn-ghost btn-icon-sm" onClick={copyConfig} title="Copy config">
                      {copied ? <Check size={14} style={{ color: "var(--success)" }} /> : <Copy size={14} />}
                    </button>
                    <button className="btn btn-secondary btn-sm" onClick={() => startEdit(selected)}>
                      <Edit3 size={14} />
                      {i.mcp.editConfig}
                    </button>
                  </div>
                </div>

                {/* Save success indicator */}
                {saveSuccess && (
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 8,
                      marginBottom: 16,
                      padding: "8px 12px",
                      borderRadius: 6,
                      background: "var(--success-subtle)",
                    }}
                  >
                    <Check size={14} style={{ color: "var(--success)" }} />
                    <span style={{ fontSize: 12, color: "var(--success)", fontWeight: 500 }}>
                      {zh ? "已保存到配置文件" : "Saved to config file"}
                    </span>
                  </div>
                )}

                <div style={{ display: "flex", flexDirection: "column", gap: 18 }}>
                  {/* Status badges */}
                  <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                    <span
                      className={`badge ${selected.status === "active" ? "badge-success" : selected.status === "error" ? "badge-danger" : "badge-muted"}`}
                    >
                      {selected.status === "active" ? i.mcp.active : i.mcp.disabled}
                    </span>
                    <span className="badge badge-muted">{selected.transport}</span>
                    <span className={`badge ${getSourceBadge(selected.source)}`}>
                      {getSourceLabel(selected.source)}
                    </span>
                  </div>

                  {/* Config Path */}
                  {selected.config_path && (
                    <div>
                      <span className="field-label">{i.mcp.configPath}</span>
                      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                        <FileText size={13} style={{ color: "var(--text-secondary)", flexShrink: 0 }} />
                        <span
                          style={{
                            fontSize: 11,
                            fontFamily: "'JetBrains Mono', monospace",
                            color: "var(--text-secondary)",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {selected.config_path}
                        </span>
                      </div>
                    </div>
                  )}

                  {/* Health Status */}
                  {healthResults[selected.id] &&
                    (() => {
                      const h = healthResults[selected.id];
                      return (
                        <div>
                          <span className="field-label">{i.mcp.healthStatus}</span>
                          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                              <span
                                className={`badge ${h.status === "healthy" ? "badge-success" : h.status === "unhealthy" ? "badge-danger" : "badge-muted"}`}
                              >
                                {h.status === "healthy"
                                  ? i.mcp.healthy
                                  : h.status === "unhealthy"
                                    ? i.mcp.unhealthy
                                    : i.mcp.unknown}
                              </span>
                              <span className={`badge ${h.command_exists ? "badge-success" : "badge-danger"}`}>
                                {i.mcp.commandExists}: {h.command_exists ? "✓" : "✗"}
                              </span>
                              {h.latency_ms != null && (
                                <span className="badge badge-muted">
                                  {i.mcp.latency}: {h.latency_ms}ms
                                </span>
                              )}
                            </div>
                            {h.error_message && (
                              <div className="code-block" style={{ fontSize: 11, color: "var(--danger)" }}>
                                {h.error_message}
                              </div>
                            )}
                          </div>
                        </div>
                      );
                    })()}

                  {/* Command */}
                  <div>
                    <span className="field-label">{i.mcp.command}</span>
                    <div className="code-block" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}>
                      {selected.command || i.common.na}
                    </div>
                  </div>

                  {/* Arguments */}
                  <div>
                    <span className="field-label">{i.mcp.arguments}</span>
                    <CodeEditor
                      value={formatJson(selected.args)}
                      language="json"
                      readOnly
                      minHeight={80}
                      maxHeight={180}
                    />
                  </div>

                  {/* Environment */}
                  <div>
                    <span className="field-label">{i.mcp.environment}</span>
                    <CodeEditor
                      value={(() => {
                        try {
                          const e = JSON.parse(selected.env);
                          return Object.keys(e).length ? JSON.stringify(e, null, 2) : "{}";
                        } catch {
                          return selected.env;
                        }
                      })()}
                      language="json"
                      readOnly
                      minHeight={80}
                      maxHeight={180}
                    />
                  </div>

                  {/* Sync to other tools */}
                  {installedTools.length > 0 && (
                    <div>
                      <span className="field-label" style={{ display: "flex", alignItems: "center", gap: 6 }}>
                        <Share2 size={12} />
                        {zh ? "同步到其他工具" : "Sync to other tools"}
                      </span>
                      <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                        {installedTools.map((tool) => {
                          const isSynced = toolSyncStatus[tool.id] || false;
                          return (
                            <button
                              key={tool.id}
                              className={`btn btn-xs ${isSynced ? "btn-primary" : "btn-secondary"}`}
                              style={{ gap: 4, textTransform: "capitalize" }}
                              disabled={syncingTo === tool.id}
                              onClick={() => toggleToolSync(tool.id)}
                            >
                              {syncingTo === tool.id ? (
                                <div className="spinner" style={{ width: 11, height: 11 }} />
                              ) : isSynced ? (
                                <Check size={11} />
                              ) : (
                                <Share2 size={11} />
                              )}
                              {tool.name}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </div>
              </div>
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
