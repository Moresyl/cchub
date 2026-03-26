import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Globe, FolderOpen, Info, Palette, Sun, Moon, Download, RefreshCw, CheckCircle, AlertCircle, Copy, Check, Upload, Archive, Wifi, Link2 } from "lucide-react";
import { t, getLocale, setLocale, type Locale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import { getTheme, setTheme, type Theme } from "../lib/theme";
import { checkAppUpdate, installAppUpdate, type AppUpdateHandle, type AppUpdateResult } from "../lib/appUpdater";
import { getVersion } from "@tauri-apps/api/app";

interface CustomPath { tool_id: string; config_dir: string | null; mcp_config_path: string | null; skills_dir: string | null; }
interface DetectedTool { id: string; name: string; config_path: string; skills_dir: string; mcp_config_path: string; installed: boolean; install_command: string; install_url: string; }
interface PendingImportedProjectRoot { project_root: string; file_count: number; }
interface AutoRemapImportedProjectRootsResult { remapped_roots: number; restored_files: number; skipped_roots: number; }
interface LastImportSummary {
  imported_at: string;
  db_rows_restored: number;
  tool_configs_restored: number;
  skills_restored: number;
  full_files_restored: number;
  pending_project_files: number;
  safety_backup_path: string;
}
interface FullRescanResult {
  mcp_servers: number;
  skills: number;
  hooks: number;
  instruction_files: number;
  workflows: number;
  config_roots: number;
  pending_project_roots: number;
  tool_health_issues: number;
  manual_setup_required: number;
}
interface ToolEnvironmentReport {
  tool_id: string;
  tool_name: string;
  cli_available: boolean;
  cli_command: string;
  config_path: string;
  config_exists: boolean;
  mcp_config_path: string;
  mcp_config_exists: boolean;
  skills_dir: string;
  skills_dir_exists: boolean;
  config_dir: string;
  config_dir_exists: boolean;
  has_custom_config_dir: boolean;
  has_custom_mcp_config_path: boolean;
  has_custom_skills_dir: boolean;
  manual_setup_kind: string | null;
  manual_setup_command: string | null;
  manual_setup_path: string | null;
}
interface BootstrapToolEnvironmentResult {
  created_dirs: number;
  created_files: number;
  notes: string[];
}
interface RepairAllResult {
  remapped_roots: number;
  restored_project_files: number;
  skipped_remap_roots: number;
  bootstrapped_tools: number;
  created_dirs: number;
  created_files: number;
  bootstrap_notes: string[];
  rescan: FullRescanResult;
}

function hasToolHealthIssue(report: ToolEnvironmentReport) {
  return !report.cli_available
    || !report.config_dir_exists
    || !report.config_exists
    || !report.mcp_config_exists
    || !report.skills_dir_exists;
}

export default function Settings() {
  const [locale, setLoc] = useState<Locale>(getLocale());
  const [theme, setThm] = useState<Theme>(getTheme());
  const [autoScan, setAutoScan] = useState(true);
  const [checkUpdates, setCheckUpdates] = useState(true);
  const [appUpdate, setAppUpdate] = useState<AppUpdateResult | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [updateHandle, setUpdateHandle] = useState<AppUpdateHandle | null>(null);
  const [appVersion, setAppVersion] = useState("");
  const [tools, setTools] = useState<DetectedTool[]>([]);
  const [customPaths, setCustomPaths] = useState<CustomPath[]>([]);
  const [pathSaved, setPathSaved] = useState<string | null>(null);
  const [proxyUrl, setProxyUrl] = useState("");
  const [proxySaved, setProxySaved] = useState(false);
  const [skillSyncMethod, setSkillSyncMethod] = useState<"symlink" | "copy">("copy");
  const [pendingProjectRoots, setPendingProjectRoots] = useState<PendingImportedProjectRoot[]>([]);
  const [toolReports, setToolReports] = useState<ToolEnvironmentReport[]>([]);
  const [lastImportSummary, setLastImportSummary] = useState<LastImportSummary | null>(null);
  const [lastRescan, setLastRescan] = useState<FullRescanResult | null>(null);
  const [remapTargets, setRemapTargets] = useState<Record<string, string>>({});
  const [remappingRoot, setRemappingRoot] = useState<string | null>(null);
  const [autoMatchingPending, setAutoMatchingPending] = useState(false);
  const [bootstrappingToolId, setBootstrappingToolId] = useState<string | null>(null);
  const [repairingAll, setRepairingAll] = useState(false);
  const [rescanningAll, setRescanningAll] = useState(false);
  const [refreshingMigrationHealth, setRefreshingMigrationHealth] = useState(false);
  const [exportingBackup, setExportingBackup] = useState(false);
  const [importingBackup, setImportingBackup] = useState(false);
  const [migrationPanelsOpen, setMigrationPanelsOpen] = useState({
    summary: false,
    pending: false,
    health: false,
    auth: false,
  });
  const migrationPanelsInitialized = useRef(false);
  const migrationPanelRefs = {
    summary: useRef<HTMLDetailsElement | null>(null),
    pending: useRef<HTMLDetailsElement | null>(null),
    health: useRef<HTMLDetailsElement | null>(null),
    auth: useRef<HTMLDetailsElement | null>(null),
  };
  const i = t();
  const loc = getLocale();

  useEffect(() => {
    loadToolsAndPaths();
    loadProxy();
    loadSkillSyncMethod();
    loadPendingProjectRoots();
    loadLastImportSummary();
    getVersion().then(v => setAppVersion("v" + v)).catch(() => {});
  }, []);

  async function loadProxy() {
    try {
      const proxy = await invoke<string>("get_proxy");
      setProxyUrl(proxy);
    } catch { /* ignore */ }
  }

  async function loadSkillSyncMethod() {
    try {
      const method = await invoke<string>("get_skill_sync_method");
      if (method === "symlink" || method === "copy") setSkillSyncMethod(method);
    } catch { /* ignore */ }
  }

  async function loadPendingProjectRoots() {
    try {
      const roots = await invoke<PendingImportedProjectRoot[]>("get_pending_imported_project_roots");
      applyPendingProjectRoots(roots);
    } catch { /* ignore */ }
  }

  async function loadLastImportSummary() {
    try {
      const summary = await invoke<LastImportSummary | null>("get_last_import_summary");
      setLastImportSummary(summary);
    } catch { /* ignore */ }
  }

  async function handleSyncMethodChange(method: "symlink" | "copy") {
    try {
      await invoke("set_skill_sync_method", { method });
      setSkillSyncMethod(method);
      showToast("success", loc === "zh" ? "已保存" : "Saved");
    } catch (e) { showToast("error", String(e)); }
  }

  async function loadToolsAndPaths() {
    try {
      const [t, p, reports] = await Promise.all([
        invoke<DetectedTool[]>("detect_tools"),
        invoke<CustomPath[]>("get_custom_paths"),
        invoke<ToolEnvironmentReport[]>("get_tool_environment_report"),
      ]);
      setTools(t);
      setCustomPaths(p);
      setToolReports(reports);
    } catch (e) { console.error(e); }
  }

  async function refreshMigrationState() {
    await Promise.allSettled([
      loadToolsAndPaths(),
      loadPendingProjectRoots(),
      loadLastImportSummary(),
      invoke("sync_config_profiles"),
    ]);
  }

  function applyPendingProjectRoots(roots: PendingImportedProjectRoot[]) {
    setPendingProjectRoots(roots);
    setRemapTargets((current) => {
      const next: Record<string, string> = {};
      for (const item of roots) {
        next[item.project_root] = current[item.project_root] || "";
      }
      return next;
    });
  }

  async function fetchMigrationStatusCounts() {
    const [roots, reports] = await Promise.all([
      invoke<PendingImportedProjectRoot[]>("get_pending_imported_project_roots"),
      invoke<ToolEnvironmentReport[]>("get_tool_environment_report"),
    ]);
    applyPendingProjectRoots(roots);
    setToolReports(reports);
    return {
      pendingRoots: roots.length,
      healthIssues: reports.filter(hasToolHealthIssue).length,
      authGaps: reports.filter((report) => !!report.manual_setup_kind).length,
    };
  }

  function toggleMigrationPanel(panel: keyof typeof migrationPanelsOpen, open: boolean) {
    setMigrationPanelsOpen((current) => ({ ...current, [panel]: open }));
  }

  function focusMigrationPanel(panel: keyof typeof migrationPanelsOpen) {
    setMigrationPanelsOpen((current) => ({ ...current, [panel]: true }));
    window.setTimeout(() => {
      migrationPanelRefs[panel].current?.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 0);
  }

  async function runBootstrapForTool(toolId: string, toolName: string) {
    setBootstrappingToolId(toolId);
    try {
      const result = await invoke<BootstrapToolEnvironmentResult>("bootstrap_tool_environment", {
        toolId,
      });
      await refreshMigrationState();
      const message = [
        i.settings.migrationHealthBootstrapSuccess
          .replace("{dirs}", String(result.created_dirs))
          .replace("{files}", String(result.created_files)),
        ...result.notes,
      ].join("；");
      showToast("success", message || `${toolName} updated`);
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setBootstrappingToolId((current) => current === toolId ? null : current);
    }
  }

  async function openInSystemWithLabel(target: string, label: string) {
    try {
      await invoke("open_in_system", { target });
    } catch (e) {
      showToast(
        "error",
        `${i.settings.openFailed.replace("{label}", label)}: ${String(e)}`
      );
    }
  }

  async function copyText(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      showToast("success", i.settings.copied.replace("{label}", label));
    } catch (e) {
      showToast("error", String(e));
    }
  }

  async function handleExportBackup() {
    setExportingBackup(true);
    try {
      const path = await invoke<string>("save_backup_to_file");
      showToast("success", loc === "zh" ? `备份已保存到: ${path}` : `Backup saved to: ${path}`);
    } catch (e) {
      if (String(e) !== "Cancelled") showToast("error", String(e));
    } finally {
      setExportingBackup(false);
    }
  }

  async function handleImportBackup() {
    setImportingBackup(true);
    try {
      const msg = await invoke<string>("import_backup_from_file");
      await refreshMigrationState();
      showToast("success", msg);
    } catch (e) {
      if (String(e) !== "Cancelled") showToast("error", String(e));
    } finally {
      setImportingBackup(false);
    }
  }

  async function handleFullRescan() {
    setRescanningAll(true);
    try {
      const result = await invoke<FullRescanResult>("run_full_rescan");
      setLastRescan(result);
      await refreshMigrationState();
      showToast(
        "success",
        i.settings.fullRescanSuccess
          .replace("{mcp}", String(result.mcp_servers))
          .replace("{skills}", String(result.skills))
          .replace("{hooks}", String(result.hooks))
          .replace("{docs}", String(result.instruction_files))
      );
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setRescanningAll(false);
    }
  }

  async function handleRepairAll() {
    setRepairingAll(true);
    try {
      const result = await invoke<RepairAllResult>("repair_all_migration_issues");
      setLastRescan(result.rescan);
      await refreshMigrationState();
      const status = await fetchMigrationStatusCounts();
      showToast(
        "success",
        i.settings.pendingImportsRepairAllSuccess
          .replace("{roots}", String(result.remapped_roots))
          .replace("{files}", String(result.restored_project_files))
          .replace("{tools}", String(result.bootstrapped_tools))
          .replace("{pending}", String(status.pendingRoots))
          .replace("{issues}", String(status.healthIssues))
          .replace("{auth}", String(status.authGaps))
      );
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setRepairingAll(false);
    }
  }

  async function handleAutoMatchPending() {
    setAutoMatchingPending(true);
    try {
      const result = await invoke<AutoRemapImportedProjectRootsResult>("auto_remap_imported_project_roots");
      await refreshMigrationState();
      const status = await fetchMigrationStatusCounts();
      showToast(
        "success",
        i.settings.pendingImportsAutoMatchSuccess
          .replace("{roots}", String(result.remapped_roots))
          .replace("{files}", String(result.restored_files))
          .replace("{skipped}", String(result.skipped_roots))
          .replace("{pending}", String(status.pendingRoots))
          .replace("{issues}", String(status.healthIssues))
      );
    } catch (e) {
      showToast("error", String(e));
    } finally {
      setAutoMatchingPending(false);
    }
  }

  async function handleRefreshMigrationHealth() {
    setRefreshingMigrationHealth(true);
    try {
      const [detectedTools, savedPaths, reports] = await Promise.all([
        invoke<DetectedTool[]>("detect_tools"),
        invoke<CustomPath[]>("get_custom_paths"),
        invoke<ToolEnvironmentReport[]>("get_tool_environment_report"),
      ]);
      setTools(detectedTools);
      setCustomPaths(savedPaths);
      setToolReports(reports);
      showToast(
        "success",
        i.settings.migrationHealthRefreshSuccess.replace(
          "{count}",
          String(reports.filter(hasToolHealthIssue).length)
        )
      );
    } catch (e) {
      console.error(e);
      showToast("error", String(e));
    } finally {
      setRefreshingMigrationHealth(false);
    }
  }

  function handleLocaleChange(newLocale: Locale) {
    setLocale(newLocale);
    setLoc(newLocale);
    window.location.reload();
  }

  function handleThemeChange(newTheme: Theme) {
    setTheme(newTheme);
    setThm(newTheme);
  }

  async function handleCheckUpdate() {
    setCheckingUpdate(true);
    setUpdateError(null);
    try {
      const { result, handle } = await checkAppUpdate();
      setAppUpdate(result);
      setUpdateHandle(handle);
    } catch (e) {
      setUpdateError(String(e));
      setUpdateHandle(null);
    } finally {
      setCheckingUpdate(false);
    }
  }

  async function handleInstallUpdate() {
    if (!updateHandle) return;
    setInstalling(true);
    setUpdateError(null);
    try {
      await installAppUpdate(updateHandle);
    } catch (e) {
      setUpdateError(String(e));
    } finally {
      setInstalling(false);
    }
  }

  const toolMeta = new Map(tools.map((tool) => [tool.id, tool] as const));
  const toolHealthIssues = toolReports.filter(hasToolHealthIssue);
  const manualSetupReports = toolReports.filter((report) => !!report.manual_setup_kind);
  const pendingProjectFiles = pendingProjectRoots.reduce((sum, item) => sum + item.file_count, 0);
  const migrationReady = pendingProjectRoots.length === 0 && toolHealthIssues.length === 0 && manualSetupReports.length === 0;

  useEffect(() => {
    if (migrationPanelsInitialized.current) return;
    if (tools.length === 0 && !lastImportSummary && pendingProjectRoots.length === 0 && toolReports.length === 0) return;
    setMigrationPanelsOpen({
      summary: !!lastImportSummary,
      pending: pendingProjectRoots.length > 0,
      health: toolHealthIssues.length > 0,
      auth: manualSetupReports.length > 0,
    });
    migrationPanelsInitialized.current = true;
  }, [lastImportSummary, manualSetupReports.length, pendingProjectRoots.length, toolHealthIssues.length, toolReports.length, tools.length]);

  const migrationOverviewCards = [
    {
      panel: "pending" as const,
      label: i.settings.pendingImports,
      value: pendingProjectRoots.length,
      tone: pendingProjectRoots.length > 0 ? "warning" : "ready",
      helper: pendingProjectRoots.length > 0
        ? (loc === "zh" ? "需要恢复路径" : "Needs path repair")
        : (loc === "zh" ? "已处理" : "Resolved"),
    },
    {
      panel: "summary" as const,
      label: i.settings.importSummaryPending,
      value: pendingProjectFiles,
      tone: pendingProjectFiles > 0 ? "warning" : "neutral",
      helper: lastImportSummary
        ? (loc === "zh" ? "查看最近导入" : "Review latest import")
        : (loc === "zh" ? "暂无导入记录" : "No recent import"),
    },
    {
      panel: "health" as const,
      label: i.settings.migrationHealth,
      value: toolHealthIssues.length,
      tone: toolHealthIssues.length > 0 ? "danger" : "ready",
      helper: toolHealthIssues.length > 0
        ? (loc === "zh" ? "优先处理环境缺失" : "Fix environment gaps first")
        : (loc === "zh" ? "环境正常" : "Environment ready"),
    },
    {
      panel: "auth" as const,
      label: i.settings.authGuide,
      value: manualSetupReports.length,
      tone: manualSetupReports.length > 0 ? "warning" : "ready",
      helper: manualSetupReports.length > 0
        ? (loc === "zh" ? "仍需手动认证" : "Manual auth still required")
        : (loc === "zh" ? "无需补全" : "No manual auth needed"),
    },
  ];

  return (
    <div className="animate-in">
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.settings.title}</h2>
        </div>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
        {/* Appearance */}
        <div className="section-card">
          <div className="section-card-title">
            <Palette size={17} style={{ color: "var(--text-secondary)" }} />
            {i.settings.appearance}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
            {/* Theme */}
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.theme}</p>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                {([["dark", i.settings.dark, Moon], ["light", i.settings.light, Sun]] as [Theme, string, typeof Moon][]).map(([key, label, Icon]) => (
                  <button
                    key={key}
                    className={`btn btn-sm ${theme === key ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => handleThemeChange(key)}
                    style={{ gap: 6 }}
                  >
                    <Icon size={14} />{label}
                  </button>
                ))}
              </div>
            </div>

            <div className="divider" />

            {/* Language */}
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <Globe size={16} style={{ color: "var(--text-secondary)" }} />
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.language}</p>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                {([["zh", "中文"], ["en", "English"]] as [Locale, string][]).map(([key, label]) => (
                  <button
                    key={key}
                    className={`btn btn-sm ${locale === key ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => handleLocaleChange(key)}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* General */}
        <div className="section-card">
          <div className="section-card-title">{i.settings.general}</div>

          <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.autoScan}</p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{i.settings.autoScanDesc}</p>
              </div>
              <button className={`toggle ${autoScan ? "on" : "off"}`} onClick={() => setAutoScan(!autoScan)}>
                <div className="toggle-knob" />
              </button>
            </div>

            <div className="divider" />

            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.checkUpdates}</p>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{i.settings.checkUpdatesDesc}</p>
              </div>
              <button className={`toggle ${checkUpdates ? "on" : "off"}`} onClick={() => setCheckUpdates(!checkUpdates)}>
                <div className="toggle-knob" />
              </button>
            </div>

            <div className="divider" />

            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <Link2 size={15} style={{ color: "var(--text-secondary)" }} />
                  <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.skillSyncMethod}</p>
                </div>
                <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{i.settings.skillSyncDesc}</p>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                {(["symlink", "copy"] as const).map((m) => (
                  <button
                    key={m}
                    className={`btn btn-sm ${skillSyncMethod === m ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => handleSyncMethodChange(m)}
                  >
                    {m === "symlink" ? i.settings.skillSyncSymlink : i.settings.skillSyncCopy}
                  </button>
                ))}
              </div>
            </div>
            {skillSyncMethod === "symlink" && (
              <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: -8 }}>
                {i.settings.skillSyncSymlinkHint}
              </p>
            )}
          </div>
        </div>

        {/* Tool Paths */}
        <div className="section-card">
          <div className="section-card-title">
            <FolderOpen size={17} style={{ color: "var(--text-secondary)" }} />
            {loc === "zh" ? "工具路径配置" : "Tool Path Configuration"}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            {tools.map((tool) => {
              const custom = customPaths.find(p => p.tool_id === tool.id);
              return (
                <div key={tool.id} style={{ padding: "12px 16px", borderRadius: 8, background: "var(--bg-input)", opacity: tool.installed ? 1 : 0.6 }}>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <span style={{ fontSize: 13, fontWeight: 600 }}>{tool.name}</span>
                      <span className={`badge ${tool.installed ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
                        {tool.installed ? (loc === "zh" ? "已安装" : "Installed") : (loc === "zh" ? "未安装" : "Not installed")}
                      </span>
                    </div>
                    {pathSaved === tool.id && <Check size={14} style={{ color: "var(--success)" }} />}
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      <span style={{ fontSize: 11, color: "var(--text-muted)", width: 80, flexShrink: 0 }}>MCP</span>
                      <input
                        className="input"
                        style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace", padding: "4px 8px", height: 28, flex: 1 }}
                        defaultValue={custom?.mcp_config_path || tool.mcp_config_path}
                        onBlur={async (e) => {
                          const val = e.target.value.trim();
                          if (val && val !== tool.mcp_config_path) {
                            await invoke("save_custom_path", { toolId: tool.id, configDir: custom?.config_dir || null, mcpConfigPath: val, skillsDir: custom?.skills_dir || null });
                            setPathSaved(tool.id); setTimeout(() => setPathSaved(null), 2000);
                            loadToolsAndPaths();
                          }
                        }}
                      />
                      <button className="btn btn-ghost btn-icon-sm" title={loc === "zh" ? "选择文件" : "Pick file"}
                        onClick={async () => {
                          const picked = await invoke<string | null>("pick_file");
                          if (picked) {
                            await invoke("save_custom_path", { toolId: tool.id, configDir: custom?.config_dir || null, mcpConfigPath: picked, skillsDir: custom?.skills_dir || null });
                            setPathSaved(tool.id); setTimeout(() => setPathSaved(null), 2000);
                            loadToolsAndPaths();
                          }
                        }}>
                        <FolderOpen size={12} />
                      </button>
                    </div>
                    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                      <span style={{ fontSize: 11, color: "var(--text-muted)", width: 80, flexShrink: 0 }}>Skills</span>
                      <input
                        className="input"
                        style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace", padding: "4px 8px", height: 28, flex: 1 }}
                        defaultValue={custom?.skills_dir || tool.skills_dir}
                        onBlur={async (e) => {
                          const val = e.target.value.trim();
                          if (val && val !== tool.skills_dir) {
                            await invoke("save_custom_path", { toolId: tool.id, configDir: custom?.config_dir || null, mcpConfigPath: custom?.mcp_config_path || null, skillsDir: val });
                            setPathSaved(tool.id); setTimeout(() => setPathSaved(null), 2000);
                            loadToolsAndPaths();
                          }
                        }}
                      />
                      <button className="btn btn-ghost btn-icon-sm" title={loc === "zh" ? "选择文件夹" : "Pick folder"}
                        onClick={async () => {
                          const picked = await invoke<string | null>("pick_folder");
                          if (picked) {
                            await invoke("save_custom_path", { toolId: tool.id, configDir: custom?.config_dir || null, mcpConfigPath: custom?.mcp_config_path || null, skillsDir: picked });
                            setPathSaved(tool.id); setTimeout(() => setPathSaved(null), 2000);
                            loadToolsAndPaths();
                          }
                        }}>
                        <FolderOpen size={12} />
                      </button>
                    </div>
                  </div>
                  {!tool.installed && tool.install_command && (
                    <div style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 6 }}>
                      <code style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>{tool.install_command}</code>
                      <button
                        className="btn btn-ghost btn-icon-sm"
                        onClick={() => copyText(tool.install_command, loc === "zh" ? `${tool.name} 安装命令` : `${tool.name} install command`)}
                        title="Copy"
                      >
                        <Copy size={11} />
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        <div className="section-card">
          <div className="section-card-title">
            <Archive size={17} style={{ color: "var(--text-secondary)" }} />
            {i.settings.migrationCenter}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
            {i.settings.migrationCenterDesc}
          </p>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))", gap: 10, marginBottom: 16 }}>
            {migrationOverviewCards.map(({ panel, label, value, tone, helper }) => {
              const isActive = migrationPanelsOpen[panel];
              const palette = tone === "danger"
                ? {
                    border: "rgba(239, 68, 68, 0.35)",
                    background: "rgba(239, 68, 68, 0.08)",
                    valueColor: "var(--error)",
                    badgeBg: "rgba(239, 68, 68, 0.14)",
                    badgeColor: "var(--error)",
                  }
                : tone === "warning"
                  ? {
                      border: "rgba(245, 158, 11, 0.35)",
                      background: "rgba(245, 158, 11, 0.08)",
                      valueColor: "var(--warning)",
                      badgeBg: "rgba(245, 158, 11, 0.14)",
                      badgeColor: "var(--warning)",
                    }
                  : tone === "ready"
                    ? {
                        border: "rgba(34, 197, 94, 0.28)",
                        background: "rgba(34, 197, 94, 0.07)",
                        valueColor: "var(--success)",
                        badgeBg: "rgba(34, 197, 94, 0.12)",
                        badgeColor: "var(--success)",
                      }
                    : {
                        border: "var(--border-color)",
                        background: "var(--bg-input)",
                        valueColor: "var(--text-primary)",
                        badgeBg: "var(--bg-card)",
                        badgeColor: "var(--text-secondary)",
                      };

              return (
              <button
                key={label}
                type="button"
                onClick={() => focusMigrationPanel(panel)}
                style={{
                  padding: "12px 14px",
                  borderRadius: 10,
                  background: palette.background,
                  border: `1px solid ${palette.border}`,
                  textAlign: "left",
                  cursor: "pointer",
                  boxShadow: isActive ? "0 0 0 1px var(--accent-primary)" : "none",
                  transform: isActive ? "translateY(-1px)" : "none",
                  transition: "all 160ms ease",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "flex-start", marginBottom: 10 }}>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.4 }}>{label}</div>
                  <span
                    style={{
                      fontSize: 10,
                      padding: "3px 7px",
                      borderRadius: 999,
                      background: palette.badgeBg,
                      color: palette.badgeColor,
                      whiteSpace: "nowrap",
                    }}
                  >
                    {isActive ? (loc === "zh" ? "当前展开" : "Open") : (loc === "zh" ? "查看" : "View")}
                  </span>
                </div>
                <div style={{ fontSize: 22, fontWeight: 700, lineHeight: 1.1, color: palette.valueColor }}>{value}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>{helper}</div>
              </button>
              );
            })}
          </div>

          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
            <button
              className="btn btn-primary btn-sm"
              style={{ gap: 6 }}
              onClick={handleExportBackup}
              disabled={exportingBackup}
            >
              <Download size={14} className={exportingBackup ? "spin" : ""} />
              {exportingBackup ? i.settings.migrationExporting : i.settings.migrationExport}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              style={{ gap: 6 }}
              onClick={handleImportBackup}
              disabled={importingBackup}
            >
              <Upload size={14} className={importingBackup ? "spin" : ""} />
              {importingBackup ? i.settings.migrationImporting : i.settings.migrationImport}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              style={{ gap: 6 }}
              disabled={repairingAll}
              onClick={handleRepairAll}
            >
              <RefreshCw size={14} className={repairingAll ? "spin" : ""} />
              {repairingAll ? i.settings.pendingImportsRepairingAll : i.settings.pendingImportsRepairAll}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              style={{ gap: 6 }}
              disabled={rescanningAll}
              onClick={handleFullRescan}
            >
              <RefreshCw size={14} className={rescanningAll ? "spin" : ""} />
              {rescanningAll ? i.settings.fullRescanning : i.settings.fullRescan}
            </button>
          </div>

          {migrationReady ? (
            <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
              <CheckCircle size={16} style={{ color: "var(--success)" }} />
              {i.settings.migrationCenterReady}
            </div>
          ) : (
            <div />
          )}
          <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 16 }}>
            <details
              ref={migrationPanelRefs.summary}
              open={migrationPanelsOpen.summary}
              onToggle={(event) => toggleMigrationPanel("summary", event.currentTarget.open)}
              style={{ borderRadius: 10, background: "var(--bg-input)" }}
            >
              <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
                {i.settings.importSummary}
              </summary>
              <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
                {lastImportSummary ? (
                  <>
                    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))", gap: 8 }}>
                      {[
                        [i.settings.importSummaryImportedAt, lastImportSummary.imported_at],
                        [i.settings.importSummaryData, String(lastImportSummary.db_rows_restored)],
                        [i.settings.importSummaryToolConfigs, String(lastImportSummary.tool_configs_restored)],
                        [i.settings.importSummarySkills, String(lastImportSummary.skills_restored)],
                        [i.settings.importSummaryFiles, String(lastImportSummary.full_files_restored)],
                        [i.settings.importSummaryPending, String(lastImportSummary.pending_project_files)],
                      ].map(([label, value]) => (
                        <div key={String(label)} style={{ padding: "8px 10px", borderRadius: 8, background: "var(--bg-card)" }}>
                          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{label}</div>
                          <div style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-word" }}>{value}</div>
                        </div>
                      ))}
                    </div>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
                      <div style={{ minWidth: 0 }}>
                        <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{i.settings.importSummaryBackup}</div>
                        <div style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace", color: "var(--text-secondary)", wordBreak: "break-all" }}>
                          {lastImportSummary.safety_backup_path}
                        </div>
                      </div>
                      <button
                        className="btn btn-secondary btn-sm"
                        type="button"
                        onClick={() => openInSystemWithLabel(
                          lastImportSummary.safety_backup_path,
                          loc === "zh" ? "安全备份路径" : "safety backup path"
                        )}
                        style={{ gap: 6 }}
                      >
                        <FolderOpen size={14} />
                        {i.settings.authGuideOpenPath}
                      </button>
                    </div>
                  </>
                ) : (
                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.importSummaryEmpty}</div>
                )}

                <div style={{ paddingTop: 4 }}>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 6 }}>{i.settings.fullRescan}</div>
                  {lastRescan ? (
                    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(120px, 1fr))", gap: 8 }}>
                      {[
                        [i.settings.rescanMcp, lastRescan.mcp_servers],
                        [i.settings.rescanSkills, lastRescan.skills],
                        [i.settings.rescanHooks, lastRescan.hooks],
                        [i.settings.rescanDocs, lastRescan.instruction_files],
                        [i.settings.rescanWorkflows, lastRescan.workflows],
                        [i.settings.rescanConfigRoots, lastRescan.config_roots],
                      ].map(([label, value]) => (
                        <div key={String(label)} style={{ fontSize: 12 }}>
                          <span style={{ color: "var(--text-muted)" }}>{label}: </span>
                          <span style={{ fontFamily: "'JetBrains Mono', monospace" }}>{value}</span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.migrationCenterLastRescanEmpty}</div>
                  )}
                </div>
              </div>
            </details>

            <details
              ref={migrationPanelRefs.pending}
              open={migrationPanelsOpen.pending}
              onToggle={(event) => toggleMigrationPanel("pending", event.currentTarget.open)}
              style={{ borderRadius: 10, background: "var(--bg-input)" }}
            >
              <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
                {i.settings.pendingImports}
              </summary>
              <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
                <div style={{ display: "flex", gap: 12, justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap" }}>
                  <div>
                    <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.pendingImportsDesc}</p>
                    <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>{i.settings.pendingImportsAutoMatchDesc}</p>
                  </div>
                  <button
                    className="btn btn-secondary btn-sm"
                    type="button"
                    disabled={autoMatchingPending || pendingProjectRoots.length === 0}
                    onClick={handleAutoMatchPending}
                  >
                    <RefreshCw size={14} className={autoMatchingPending ? "spin" : ""} />
                    {autoMatchingPending ? i.settings.pendingImportsAutoMatching : i.settings.pendingImportsAutoMatch}
                  </button>
                </div>
                {pendingProjectRoots.length === 0 ? (
                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.pendingImportsEmpty}</div>
                ) : (
                  pendingProjectRoots.map((item) => (
                    <div
                      key={item.project_root}
                      style={{
                        padding: "12px 14px",
                        borderRadius: 10,
                        background: "var(--bg-card)",
                        display: "flex",
                        flexDirection: "column",
                        gap: 10,
                      }}
                    >
                      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
                        <div style={{ minWidth: 0 }}>
                          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{i.settings.pendingImportsOldPath}</div>
                          <div style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>{item.project_root}</div>
                        </div>
                        <span className="badge badge-muted">{i.settings.pendingImportsFiles.replace("{count}", String(item.file_count))}</span>
                      </div>
                      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                        <input
                          className="input"
                          style={{ flex: 1, minWidth: 220, fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
                          placeholder={i.settings.pendingImportsNewPath}
                          value={remapTargets[item.project_root] || ""}
                          onChange={(e) => setRemapTargets((current) => ({ ...current, [item.project_root]: e.target.value }))}
                        />
                        <button
                          className="btn btn-secondary btn-sm"
                          type="button"
                          onClick={async () => {
                            try {
                              const picked = await invoke<string | null>("pick_folder");
                              if (picked) {
                                setRemapTargets((current) => ({ ...current, [item.project_root]: picked }));
                              }
                            } catch (e) { console.error(e); }
                          }}
                        >
                          <FolderOpen size={14} />
                          {i.settings.pendingImportsPick}
                        </button>
                        <button
                          className="btn btn-primary btn-sm"
                          type="button"
                          disabled={remappingRoot === item.project_root || !(remapTargets[item.project_root] || "").trim()}
                          onClick={async () => {
                            const targetPath = (remapTargets[item.project_root] || "").trim();
                            if (!targetPath) return;
                            setRemappingRoot(item.project_root);
                            try {
                              const restored = await invoke<number>("remap_imported_project_root", {
                                sourcePath: item.project_root,
                                targetPath,
                              });
                              await refreshMigrationState();
                              showToast(
                                "success",
                                i.settings.pendingImportsSuccess
                                  .replace("{count}", String(restored))
                                  .replace("{target}", targetPath)
                              );
                            } catch (e) {
                              showToast("error", String(e));
                            } finally {
                              setRemappingRoot((current) => current === item.project_root ? null : current);
                            }
                          }}
                        >
                          {remappingRoot === item.project_root ? i.settings.pendingImportsApplying : i.settings.pendingImportsApply}
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </details>

            <details
              ref={migrationPanelRefs.health}
              open={migrationPanelsOpen.health}
              onToggle={(event) => toggleMigrationPanel("health", event.currentTarget.open)}
              style={{ borderRadius: 10, background: "var(--bg-input)" }}
            >
              <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
                {i.settings.migrationHealth}
              </summary>
              <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start", flexWrap: "wrap" }}>
                  <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.migrationHealthDesc}</p>
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={handleRefreshMigrationHealth}
                    disabled={refreshingMigrationHealth}
                    style={{ gap: 6 }}
                  >
                    <RefreshCw size={14} className={refreshingMigrationHealth ? "spin" : ""} />
                    {refreshingMigrationHealth ? i.settings.migrationHealthRefreshing : i.settings.migrationHealthRefresh}
                  </button>
                </div>
                {toolHealthIssues.length === 0 ? (
                  <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
                    <CheckCircle size={16} style={{ color: "var(--success)" }} />
                    {i.settings.migrationHealthReady}
                  </div>
                ) : (
                  toolHealthIssues.map((report) => {
                    const tool = toolMeta.get(report.tool_id);
                    const issueBadges = [
                      !report.cli_available ? i.settings.migrationHealthCliMissing : null,
                      !report.config_dir_exists ? i.settings.migrationHealthConfigDirMissing : null,
                      !report.config_exists ? i.settings.migrationHealthConfigMissing : null,
                      !report.mcp_config_exists ? i.settings.migrationHealthMcpMissing : null,
                      !report.skills_dir_exists ? i.settings.migrationHealthSkillsMissing : null,
                    ].filter(Boolean) as string[];

                    return (
                      <div
                        key={report.tool_id}
                        style={{
                          padding: "12px 14px",
                          borderRadius: 10,
                          background: "var(--bg-card)",
                          display: "flex",
                          flexDirection: "column",
                          gap: 10,
                        }}
                      >
                        <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
                          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                            <span style={{ fontSize: 13, fontWeight: 600 }}>{report.tool_name}</span>
                            {issueBadges.map((badge) => (
                              <span key={badge} className="badge badge-muted" style={{ fontSize: 10 }}>{badge}</span>
                            ))}
                          </div>
                          {!report.cli_available && tool?.install_command && (
                            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                              <button
                                className="btn btn-ghost btn-sm"
                                onClick={() => copyText(
                                  tool.install_command,
                                  loc === "zh" ? `${report.tool_name} 安装命令` : `${report.tool_name} install command`
                                )}
                                style={{ gap: 6 }}
                              >
                                <Copy size={12} />
                                {i.settings.migrationHealthInstall}
                              </button>
                              <button
                                className="btn btn-secondary btn-sm"
                                onClick={() => runBootstrapForTool(report.tool_id, report.tool_name)}
                                disabled={bootstrappingToolId === report.tool_id}
                                style={{ gap: 6 }}
                              >
                                <FolderOpen size={12} className={bootstrappingToolId === report.tool_id ? "spin" : ""} />
                                {bootstrappingToolId === report.tool_id ? i.settings.migrationHealthBootstrapping : i.settings.migrationHealthBootstrap}
                              </button>
                            </div>
                          )}
                          {report.cli_available && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => runBootstrapForTool(report.tool_id, report.tool_name)}
                              disabled={bootstrappingToolId === report.tool_id}
                              style={{ gap: 6 }}
                            >
                              <FolderOpen size={12} className={bootstrappingToolId === report.tool_id ? "spin" : ""} />
                              {bootstrappingToolId === report.tool_id ? i.settings.migrationHealthBootstrapping : i.settings.migrationHealthBootstrap}
                            </button>
                          )}
                        </div>
                        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: 8 }}>
                          <div style={{ fontSize: 12 }}>
                            <div style={{ color: "var(--text-muted)", marginBottom: 4 }}>{i.settings.migrationHealthCli}</div>
                            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                              <span className={`badge ${report.cli_available ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
                                {report.cli_available ? i.settings.migrationHealthStatusOk : i.settings.migrationHealthStatusMissing}
                              </span>
                              <span style={{ fontFamily: "'JetBrains Mono', monospace" }}>{report.cli_command}</span>
                            </div>
                          </div>
                          <div style={{ fontSize: 12 }}>
                            <div style={{ color: "var(--text-muted)", marginBottom: 4 }}>{i.settings.migrationHealthPath}</div>
                            <div style={{ fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all", color: "var(--text-secondary)" }}>
                              {report.config_dir}
                            </div>
                            {(report.has_custom_config_dir || report.has_custom_mcp_config_path || report.has_custom_skills_dir) && (
                              <div style={{ marginTop: 6 }}>
                                <span className="badge badge-accent" style={{ fontSize: 10 }}>{i.settings.migrationHealthCustomPath}</span>
                              </div>
                            )}
                          </div>
                        </div>
                        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 8 }}>
                          {[
                            [i.settings.migrationHealthConfigDir, report.config_dir_exists, report.config_dir],
                            [i.settings.migrationHealthConfigFile, report.config_exists, report.config_path],
                            [i.settings.migrationHealthMcpConfig, report.mcp_config_exists, report.mcp_config_path],
                            [i.settings.migrationHealthSkillsDir, report.skills_dir_exists, report.skills_dir],
                          ].map(([label, ok, path]) => (
                            <div key={`${report.tool_id}-${label}`} style={{ padding: "8px 10px", borderRadius: 8, background: "var(--bg-input)" }}>
                              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                                <span className={`badge ${ok ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
                                  {ok ? i.settings.migrationHealthStatusOk : i.settings.migrationHealthStatusMissing}
                                </span>
                                <span style={{ fontSize: 12 }}>{label}</span>
                              </div>
                              <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
                                {path}
                              </div>
                            </div>
                          ))}
                        </div>
                        {!report.cli_available && tool?.install_command && (
                          <code style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                            {tool.install_command}
                          </code>
                        )}
                      </div>
                    );
                  })
                )}
              </div>
            </details>

            <details
              ref={migrationPanelRefs.auth}
              open={migrationPanelsOpen.auth}
              onToggle={(event) => toggleMigrationPanel("auth", event.currentTarget.open)}
              style={{ borderRadius: 10, background: "var(--bg-input)" }}
            >
              <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
                {i.settings.authGuide}
              </summary>
              <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
                <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.settings.authGuideDesc}</p>
                {manualSetupReports.length === 0 ? (
                  <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
                    <CheckCircle size={16} style={{ color: "var(--success)" }} />
                    {i.settings.authGuideReady}
                  </div>
                ) : (
                  manualSetupReports.map((report) => {
                    const tool = toolMeta.get(report.tool_id);
                    const description = report.manual_setup_kind === "codex_login"
                      ? i.settings.authGuideCodexLogin
                      : report.manual_setup_kind === "gemini_api_key"
                        ? i.settings.authGuideGeminiKey
                        : report.manual_setup_kind || "";

                    return (
                      <div
                        key={`${report.tool_id}-auth`}
                        style={{
                          padding: "12px 14px",
                          borderRadius: 10,
                          background: "var(--bg-card)",
                          display: "flex",
                          flexDirection: "column",
                          gap: 10,
                        }}
                      >
                        <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}>
                          <div>
                            <div style={{ fontSize: 13, fontWeight: 600 }}>{report.tool_name}</div>
                            <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>{description}</div>
                          </div>
                          <span className="badge badge-muted">{report.tool_id}</span>
                        </div>
                        {report.manual_setup_path && (
                          <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
                            {report.manual_setup_path}
                          </div>
                        )}
                        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                          {report.manual_setup_command && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => copyText(
                                report.manual_setup_command || "",
                                loc === "zh" ? `${report.tool_name} 认证命令` : `${report.tool_name} auth command`
                              )}
                              style={{ gap: 6 }}
                            >
                              <Copy size={12} />
                              {i.settings.authGuideCopyCommand}
                            </button>
                          )}
                          {report.manual_setup_path && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => copyText(
                                report.manual_setup_path || "",
                                loc === "zh" ? `${report.tool_name} 路径` : `${report.tool_name} path`
                              )}
                              style={{ gap: 6 }}
                            >
                              <Copy size={12} />
                              {i.settings.authGuideCopyPath}
                            </button>
                          )}
                          {report.manual_setup_path && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => openInSystemWithLabel(
                                report.manual_setup_path || "",
                                loc === "zh" ? `${report.tool_name} 路径` : `${report.tool_name} path`
                              )}
                              style={{ gap: 6 }}
                            >
                              <FolderOpen size={12} />
                              {i.settings.authGuideOpenPath}
                            </button>
                          )}
                          <button
                            className="btn btn-secondary btn-sm"
                            onClick={() => runBootstrapForTool(report.tool_id, report.tool_name)}
                            disabled={bootstrappingToolId === report.tool_id}
                            style={{ gap: 6 }}
                          >
                            <FolderOpen size={12} className={bootstrappingToolId === report.tool_id ? "spin" : ""} />
                            {bootstrappingToolId === report.tool_id ? i.settings.migrationHealthBootstrapping : i.settings.authGuidePrepareFile}
                          </button>
                          {tool?.install_url && (
                            <button
                              className="btn btn-secondary btn-sm"
                              onClick={() => openInSystemWithLabel(
                                tool.install_url,
                                loc === "zh" ? `${report.tool_name} 说明页` : `${report.tool_name} docs`
                              )}
                              style={{ gap: 6 }}
                            >
                              <Link2 size={12} />
                              {i.settings.authGuideOpenDocs}
                            </button>
                          )}
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </details>
          </div>
        </div>

        {/* App Update */}
        <div className="section-card">
          <div className="section-card-title">
            <Download size={17} style={{ color: "var(--text-secondary)" }} />
            {i.settings.appUpdate}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div>
                <p style={{ fontSize: 14, fontWeight: 500 }}>{i.settings.currentVersion}</p>
                <p style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: 2 }}>{appVersion}</p>
              </div>
              <button
                className="btn btn-sm btn-secondary"
                onClick={handleCheckUpdate}
                disabled={checkingUpdate}
                style={{ gap: 6 }}
              >
                <RefreshCw size={14} className={checkingUpdate ? "spin" : ""} />
                {checkingUpdate ? i.settings.checking : i.settings.checkForUpdate}
              </button>
            </div>

            {appUpdate && (
              <>
                <div className="divider" />
                {appUpdate.update_available ? (
                  <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <AlertCircle size={16} style={{ color: "var(--warning)" }} />
                      <span style={{ fontSize: 14, fontWeight: 500, color: "var(--warning)" }}>
                        {i.settings.updateAvailable}: v{appUpdate.latest_version}
                      </span>
                    </div>
                    {appUpdate.body && (
                      <p style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "pre-wrap" }}>{appUpdate.body}</p>
                    )}
                    {!appUpdate.can_install && (
                      <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
                        {loc === "zh"
                          ? "已通过发布页检测到新版本。当前构建未提供在线安装清单，将打开下载页手动更新。"
                          : "A newer release was found from GitHub. This build does not have an online install manifest, so the download page will be opened."}
                      </p>
                    )}
                    <button
                      className="btn btn-sm btn-primary"
                      onClick={handleInstallUpdate}
                      disabled={installing}
                      style={{ alignSelf: "flex-start", gap: 6 }}
                    >
                      <Download size={14} />
                      {installing
                        ? i.settings.downloading
                        : appUpdate.can_install
                          ? i.settings.installUpdate
                          : (loc === "zh" ? "前往下载页" : "Open Downloads")}
                    </button>
                  </div>
                ) : (
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <CheckCircle size={16} style={{ color: "var(--success)" }} />
                    <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>
                      {appUpdate.not_configured
                        ? i.settings.updateNotConfigured
                        : i.settings.noUpdate}
                    </span>
                  </div>
                )}
              </>
            )}

            {updateError && (
              <>
                <div className="divider" />
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <AlertCircle size={16} style={{ color: "var(--error)" }} />
                  <span style={{ fontSize: 13, color: "var(--error)" }}>{i.settings.updateFailed}: {updateError}</span>
                </div>
              </>
            )}
          </div>
        </div>

        {/* Network Proxy */}
        <div className="section-card">
          <div className="section-card-title">
            <Wifi size={17} style={{ color: "var(--text-secondary)" }} />
            {loc === "zh" ? "网络代理" : "Network Proxy"}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 12 }}>
            {loc === "zh"
              ? "设置 HTTP/HTTPS 代理地址，用于访问 GitHub 等外部服务。留空则使用系统默认网络。"
              : "Set HTTP/HTTPS proxy for accessing GitHub and external services. Leave empty for system default."}
          </p>
          <div style={{ display: "flex", gap: 8 }}>
            <input
              className="input"
              style={{ flex: 1, fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
              placeholder="http://127.0.0.1:7890"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && (async () => {
                try {
                  await invoke("set_proxy", { proxyUrl });
                  setProxySaved(true);
                  setTimeout(() => setProxySaved(false), 2000);
                  showToast("success", loc === "zh" ? "代理已保存" : "Proxy saved");
                } catch (e) { showToast("error", String(e)); }
              })()}
            />
            <button className="btn btn-primary btn-sm" style={{ gap: 5 }}
              onClick={async () => {
                try {
                  await invoke("set_proxy", { proxyUrl });
                  setProxySaved(true);
                  setTimeout(() => setProxySaved(false), 2000);
                  showToast("success", loc === "zh" ? (proxyUrl.trim() ? "代理已设置" : "代理已清除") : (proxyUrl.trim() ? "Proxy set" : "Proxy cleared"));
                } catch (e) { showToast("error", String(e)); }
              }}>
              {proxySaved ? <Check size={13} style={{ color: "var(--success)" }} /> : <Check size={13} />}
              {loc === "zh" ? "保存" : "Save"}
            </button>
          </div>
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
            {loc === "zh"
              ? "提示：也可以在 VPN 软件中开启 TUN 模式让所有流量走代理，无需在此设置。"
              : "Tip: You can also enable TUN mode in your VPN client to proxy all traffic without setting this."}
          </p>
        </div>

        {/* About */}
        <div className="section-card">
          <div className="section-card-title">
            <Info size={17} style={{ color: "var(--text-secondary)" }} />
            {i.settings.about}
          </div>
          <p style={{ fontSize: 13, color: "var(--text-secondary)" }}>{i.settings.aboutDesc}</p>
          <div style={{ display: "flex", gap: 12, marginTop: 12 }}>
            <span className="badge badge-muted">{appVersion}</span>
            <span className="badge badge-muted">{i.settings.license}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
