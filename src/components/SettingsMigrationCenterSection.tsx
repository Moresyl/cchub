import { memo, type RefObject } from "react";
import { Archive, CheckCircle, Download, RefreshCw, Upload } from "lucide-react";
import type { Locale } from "../lib/i18n";
import SettingsImportSummaryPanel from "./SettingsImportSummaryPanel";
import SettingsManualSetupCard from "./SettingsManualSetupCard";
import SettingsMigrationOverviewCard from "./SettingsMigrationOverviewCard";
import SettingsPendingRootCard from "./SettingsPendingRootCard";
import SettingsToolHealthCard from "./SettingsToolHealthCard";

interface MigrationPanelState {
  summary: boolean;
  pending: boolean;
  health: boolean;
  auth: boolean;
}

interface MigrationPanelRefs {
  summary: RefObject<HTMLDetailsElement | null>;
  pending: RefObject<HTMLDetailsElement | null>;
  health: RefObject<HTMLDetailsElement | null>;
  auth: RefObject<HTMLDetailsElement | null>;
}

interface PendingImportedProjectRootItem {
  project_root: string;
  file_count: number;
}

interface LastImportSummaryItem {
  imported_at: string;
  db_rows_restored: number;
  tool_configs_restored: number;
  skills_restored: number;
  full_files_restored: number;
  pending_project_files: number;
  safety_backup_path: string;
}

interface FullRescanResultItem {
  mcp_servers: number;
  skills: number;
  hooks: number;
  instruction_files: number;
  workflows: number;
  config_roots: number;
}

interface ToolEnvironmentReportItem {
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

interface DetectedToolMeta {
  id: string;
  install_command: string;
  install_url: string;
}

interface SettingsMigrationCenterSectionLabels {
  title: string;
  description: string;
  readyLabel: string;
  activeLabel: string;
  viewLabel: string;
  importSummary: string;
  importSummaryPending: string;
  pendingImports: string;
  migrationHealth: string;
  authGuide: string;
  migrationExport: string;
  migrationExporting: string;
  migrationImport: string;
  migrationImporting: string;
  pendingImportsRepairAll: string;
  pendingImportsRepairingAll: string;
  fullRescan: string;
  fullRescanning: string;
  importSummaryEmpty: string;
  importSummaryBackup: string;
  openPathLabel: string;
  migrationCenterLastRescanEmpty: string;
  importSummaryImportedAt: string;
  importSummaryData: string;
  importSummaryToolConfigs: string;
  importSummarySkills: string;
  importSummaryFiles: string;
  rescanMcp: string;
  rescanSkills: string;
  rescanHooks: string;
  rescanDocs: string;
  rescanWorkflows: string;
  rescanConfigRoots: string;
  pendingImportsDesc: string;
  pendingImportsAutoMatchDesc: string;
  pendingImportsAutoMatching: string;
  pendingImportsAutoMatch: string;
  pendingImportsEmpty: string;
  pendingImportsOldPath: string;
  pendingImportsNewPath: string;
  pendingImportsPick: string;
  pendingImportsApply: string;
  pendingImportsApplying: string;
  pendingImportsFiles: string;
  migrationHealthDesc: string;
  migrationHealthRefreshing: string;
  migrationHealthRefresh: string;
  migrationHealthReady: string;
  migrationHealthCliMissing: string;
  migrationHealthConfigDirMissing: string;
  migrationHealthConfigMissing: string;
  migrationHealthMcpMissing: string;
  migrationHealthSkillsMissing: string;
  migrationHealthInstall: string;
  migrationHealthBootstrap: string;
  migrationHealthBootstrapping: string;
  migrationHealthCli: string;
  migrationHealthPath: string;
  migrationHealthStatusOk: string;
  migrationHealthStatusMissing: string;
  migrationHealthConfigDir: string;
  migrationHealthConfigFile: string;
  migrationHealthMcpConfig: string;
  migrationHealthSkillsDir: string;
  migrationHealthCustomPath: string;
  authGuideDesc: string;
  authGuideReady: string;
  authGuideCodexLogin: string;
  authGuideGeminiKey: string;
  authGuideCopyCommand: string;
  authGuideCopyPath: string;
  authGuidePrepareFile: string;
  authGuideOpenDocs: string;
}

interface SettingsMigrationCenterSectionProps {
  locale: Locale;
  labels: SettingsMigrationCenterSectionLabels;
  tools: DetectedToolMeta[];
  pendingProjectRoots: PendingImportedProjectRootItem[];
  toolReports: ToolEnvironmentReportItem[];
  lastImportSummary: LastImportSummaryItem | null;
  lastRescan: FullRescanResultItem | null;
  remapTargets: Record<string, string>;
  remappingRoot: string | null;
  autoMatchingPending: boolean;
  bootstrappingToolId: string | null;
  repairingAll: boolean;
  rescanningAll: boolean;
  refreshingMigrationHealth: boolean;
  exportingBackup: boolean;
  importingBackup: boolean;
  migrationPanelsOpen: MigrationPanelState;
  migrationPanelRefs: MigrationPanelRefs;
  onSummaryToggle: (event: React.SyntheticEvent<HTMLDetailsElement>) => void;
  onPendingToggle: (event: React.SyntheticEvent<HTMLDetailsElement>) => void;
  onHealthToggle: (event: React.SyntheticEvent<HTMLDetailsElement>) => void;
  onAuthToggle: (event: React.SyntheticEvent<HTMLDetailsElement>) => void;
  onFocusPanel: (panel: string) => void | Promise<void>;
  onExportBackup: () => void | Promise<void>;
  onImportBackup: () => void | Promise<void>;
  onRepairAll: () => void | Promise<void>;
  onFullRescan: () => void | Promise<void>;
  onOpenBackupPath: () => void | Promise<void>;
  onAutoMatchPending: () => void | Promise<void>;
  onPendingTargetChange: (sourcePath: string, nextValue: string) => void | Promise<void>;
  onPickPendingTarget: (sourcePath: string) => void | Promise<void>;
  onApplyPendingTarget: (sourcePath: string, targetPath: string) => void | Promise<void>;
  onRefreshMigrationHealth: () => void | Promise<void>;
  onCopy: (value: string, label: string) => void | Promise<void>;
  onOpen: (target: string, label: string) => void | Promise<void>;
  onBootstrapTool: (toolId: string, toolName: string) => void | Promise<void>;
}

function hasToolHealthIssue(report: ToolEnvironmentReportItem) {
  return !report.cli_available
    || !report.config_dir_exists
    || !report.config_exists
    || !report.mcp_config_exists
    || !report.skills_dir_exists;
}

function SettingsMigrationCenterSectionComponent({
  locale,
  labels,
  tools,
  pendingProjectRoots,
  toolReports,
  lastImportSummary,
  lastRescan,
  remapTargets,
  remappingRoot,
  autoMatchingPending,
  bootstrappingToolId,
  repairingAll,
  rescanningAll,
  refreshingMigrationHealth,
  exportingBackup,
  importingBackup,
  migrationPanelsOpen,
  migrationPanelRefs,
  onSummaryToggle,
  onPendingToggle,
  onHealthToggle,
  onAuthToggle,
  onFocusPanel,
  onExportBackup,
  onImportBackup,
  onRepairAll,
  onFullRescan,
  onOpenBackupPath,
  onAutoMatchPending,
  onPendingTargetChange,
  onPickPendingTarget,
  onApplyPendingTarget,
  onRefreshMigrationHealth,
  onCopy,
  onOpen,
  onBootstrapTool,
}: SettingsMigrationCenterSectionProps) {
  const toolHealthIssues = toolReports.filter(hasToolHealthIssue);
  const manualSetupReports = toolReports.filter((report) => !!report.manual_setup_kind);
  const pendingProjectFiles = pendingProjectRoots.reduce((sum, item) => sum + item.file_count, 0);
  const migrationReady = pendingProjectRoots.length === 0 && toolHealthIssues.length === 0 && manualSetupReports.length === 0;

  const migrationOverviewCards = [
    {
      panel: "pending" as const,
      label: labels.pendingImports,
      value: pendingProjectRoots.length,
      tone: pendingProjectRoots.length > 0 ? "warning" as const : "ready" as const,
      helper: pendingProjectRoots.length > 0
        ? (locale === "zh" ? "需要恢复路径" : "Needs path repair")
        : (locale === "zh" ? "已处理" : "Resolved"),
    },
    {
      panel: "summary" as const,
      label: labels.importSummaryPending,
      value: pendingProjectFiles,
      tone: pendingProjectFiles > 0 ? "warning" as const : "neutral" as const,
      helper: lastImportSummary
        ? (locale === "zh" ? "查看最近导入" : "Review latest import")
        : (locale === "zh" ? "暂无导入记录" : "No recent import"),
    },
    {
      panel: "health" as const,
      label: labels.migrationHealth,
      value: toolHealthIssues.length,
      tone: toolHealthIssues.length > 0 ? "danger" as const : "ready" as const,
      helper: toolHealthIssues.length > 0
        ? (locale === "zh" ? "优先处理环境缺失" : "Fix environment gaps first")
        : (locale === "zh" ? "环境正常" : "Environment ready"),
    },
    {
      panel: "auth" as const,
      label: labels.authGuide,
      value: manualSetupReports.length,
      tone: manualSetupReports.length > 0 ? "warning" as const : "ready" as const,
      helper: manualSetupReports.length > 0
        ? (locale === "zh" ? "仍需手动认证" : "Manual auth still required")
        : (locale === "zh" ? "无需补全" : "No manual auth needed"),
    },
  ];

  return (
    <div className="section-card">
      <div className="section-card-title">
        <Archive size={17} style={{ color: "var(--text-secondary)" }} />
        {labels.title}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
        {labels.description}
      </p>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))", gap: 10, marginBottom: 16 }}>
        {migrationOverviewCards.map(({ panel, label, value, tone, helper }) => (
          <SettingsMigrationOverviewCard
            key={label}
            panel={panel}
            label={label}
            value={value}
            tone={tone}
            helper={helper}
            active={migrationPanelsOpen[panel]}
            activeLabel={labels.activeLabel}
            viewLabel={labels.viewLabel}
            onFocus={onFocusPanel}
          />
        ))}
      </div>

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
        <button
          className="btn btn-primary btn-sm"
          style={{ gap: 6 }}
          onClick={onExportBackup}
          disabled={exportingBackup}
        >
          <Download size={14} className={exportingBackup ? "spin" : ""} />
          {exportingBackup ? labels.migrationExporting : labels.migrationExport}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          style={{ gap: 6 }}
          onClick={onImportBackup}
          disabled={importingBackup}
        >
          <Upload size={14} className={importingBackup ? "spin" : ""} />
          {importingBackup ? labels.migrationImporting : labels.migrationImport}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          style={{ gap: 6 }}
          disabled={repairingAll}
          onClick={onRepairAll}
        >
          <RefreshCw size={14} className={repairingAll ? "spin" : ""} />
          {repairingAll ? labels.pendingImportsRepairingAll : labels.pendingImportsRepairAll}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          style={{ gap: 6 }}
          disabled={rescanningAll}
          onClick={onFullRescan}
        >
          <RefreshCw size={14} className={rescanningAll ? "spin" : ""} />
          {rescanningAll ? labels.fullRescanning : labels.fullRescan}
        </button>
      </div>

      {migrationReady ? (
        <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
          <CheckCircle size={16} style={{ color: "var(--success)" }} />
          {labels.readyLabel}
        </div>
      ) : (
        <div />
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 16 }}>
        <details
          ref={migrationPanelRefs.summary}
          open={migrationPanelsOpen.summary}
          onToggle={onSummaryToggle}
          style={{ borderRadius: 10, background: "var(--bg-input)" }}
        >
          <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
            {labels.importSummary}
          </summary>
          <SettingsImportSummaryPanel
            summary={lastImportSummary}
            rescan={lastRescan}
            emptyLabel={labels.importSummaryEmpty}
            backupLabel={labels.importSummaryBackup}
            openPathLabel={labels.openPathLabel}
            rescanTitle={labels.fullRescan}
            rescanEmptyLabel={labels.migrationCenterLastRescanEmpty}
            importedAtLabel={labels.importSummaryImportedAt}
            dataLabel={labels.importSummaryData}
            toolConfigsLabel={labels.importSummaryToolConfigs}
            skillsLabel={labels.importSummarySkills}
            filesLabel={labels.importSummaryFiles}
            pendingLabel={labels.importSummaryPending}
            mcpLabel={labels.rescanMcp}
            rescanSkillsLabel={labels.rescanSkills}
            hooksLabel={labels.rescanHooks}
            docsLabel={labels.rescanDocs}
            workflowsLabel={labels.rescanWorkflows}
            configRootsLabel={labels.rescanConfigRoots}
            onOpenBackupPath={onOpenBackupPath}
          />
        </details>

        <details
          ref={migrationPanelRefs.pending}
          open={migrationPanelsOpen.pending}
          onToggle={onPendingToggle}
          style={{ borderRadius: 10, background: "var(--bg-input)" }}
        >
          <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
            {labels.pendingImports}
          </summary>
          <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
            <div style={{ display: "flex", gap: 12, justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap" }}>
              <div>
                <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{labels.pendingImportsDesc}</p>
                <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>{labels.pendingImportsAutoMatchDesc}</p>
              </div>
              <button
                className="btn btn-secondary btn-sm"
                type="button"
                disabled={autoMatchingPending || pendingProjectRoots.length === 0}
                onClick={onAutoMatchPending}
              >
                <RefreshCw size={14} className={autoMatchingPending ? "spin" : ""} />
                {autoMatchingPending ? labels.pendingImportsAutoMatching : labels.pendingImportsAutoMatch}
              </button>
            </div>
            {pendingProjectRoots.length === 0 ? (
              <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{labels.pendingImportsEmpty}</div>
            ) : (
              pendingProjectRoots.map((item) => (
                <SettingsPendingRootCard
                  key={item.project_root}
                  item={item}
                  targetValue={remapTargets[item.project_root] || ""}
                  oldPathLabel={labels.pendingImportsOldPath}
                  newPathPlaceholder={labels.pendingImportsNewPath}
                  pickLabel={labels.pendingImportsPick}
                  applyLabel={labels.pendingImportsApply}
                  applyingLabel={labels.pendingImportsApplying}
                  filesLabel={labels.pendingImportsFiles.replace("{count}", String(item.file_count))}
                  applying={remappingRoot === item.project_root}
                  onTargetChange={onPendingTargetChange}
                  onPick={onPickPendingTarget}
                  onApply={onApplyPendingTarget}
                />
              ))
            )}
          </div>
        </details>

        <details
          ref={migrationPanelRefs.health}
          open={migrationPanelsOpen.health}
          onToggle={onHealthToggle}
          style={{ borderRadius: 10, background: "var(--bg-input)" }}
        >
          <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
            {labels.migrationHealth}
          </summary>
          <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
            <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start", flexWrap: "wrap" }}>
              <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{labels.migrationHealthDesc}</p>
              <button
                className="btn btn-secondary btn-sm"
                onClick={onRefreshMigrationHealth}
                disabled={refreshingMigrationHealth}
                style={{ gap: 6 }}
              >
                <RefreshCw size={14} className={refreshingMigrationHealth ? "spin" : ""} />
                {refreshingMigrationHealth ? labels.migrationHealthRefreshing : labels.migrationHealthRefresh}
              </button>
            </div>
            {toolHealthIssues.length === 0 ? (
              <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
                <CheckCircle size={16} style={{ color: "var(--success)" }} />
                {labels.migrationHealthReady}
              </div>
            ) : (
              toolHealthIssues.map((report) => (
                <SettingsToolHealthCard
                  key={report.tool_id}
                  report={report}
                  installCommand={tools.find((tool) => tool.id === report.tool_id)?.install_command ?? null}
                  bootstrapping={bootstrappingToolId === report.tool_id}
                  cliMissingLabel={labels.migrationHealthCliMissing}
                  configDirMissingLabel={labels.migrationHealthConfigDirMissing}
                  configMissingLabel={labels.migrationHealthConfigMissing}
                  mcpMissingLabel={labels.migrationHealthMcpMissing}
                  skillsMissingLabel={labels.migrationHealthSkillsMissing}
                  installActionLabel={labels.migrationHealthInstall}
                  bootstrapActionLabel={labels.migrationHealthBootstrap}
                  bootstrappingLabel={labels.migrationHealthBootstrapping}
                  cliLabel={labels.migrationHealthCli}
                  pathLabel={labels.migrationHealthPath}
                  statusOkLabel={labels.migrationHealthStatusOk}
                  statusMissingLabel={labels.migrationHealthStatusMissing}
                  configDirLabel={labels.migrationHealthConfigDir}
                  configFileLabel={labels.migrationHealthConfigFile}
                  mcpConfigLabel={labels.migrationHealthMcpConfig}
                  skillsDirLabel={labels.migrationHealthSkillsDir}
                  customPathLabel={labels.migrationHealthCustomPath}
                  installCommandToastLabel={locale === "zh" ? `${report.tool_name} 安装命令` : `${report.tool_name} install command`}
                  onCopy={onCopy}
                  onBootstrap={onBootstrapTool}
                />
              ))
            )}
          </div>
        </details>

        <details
          ref={migrationPanelRefs.auth}
          open={migrationPanelsOpen.auth}
          onToggle={onAuthToggle}
          style={{ borderRadius: 10, background: "var(--bg-input)" }}
        >
          <summary style={{ cursor: "pointer", listStyle: "none", padding: "12px 14px", fontSize: 13, fontWeight: 600 }}>
            {labels.authGuide}
          </summary>
          <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{labels.authGuideDesc}</p>
            {manualSetupReports.length === 0 ? (
              <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
                <CheckCircle size={16} style={{ color: "var(--success)" }} />
                {labels.authGuideReady}
              </div>
            ) : (
              manualSetupReports.map((report) => (
                <SettingsManualSetupCard
                  key={`${report.tool_id}-auth`}
                  report={report}
                  description={report.manual_setup_kind === "codex_login"
                    ? labels.authGuideCodexLogin
                    : report.manual_setup_kind === "gemini_api_key"
                      ? labels.authGuideGeminiKey
                      : report.manual_setup_kind || ""}
                  installUrl={tools.find((tool) => tool.id === report.tool_id)?.install_url ?? null}
                  bootstrapping={bootstrappingToolId === report.tool_id}
                  copyCommandLabel={labels.authGuideCopyCommand}
                  copyPathLabel={labels.authGuideCopyPath}
                  openPathLabel={labels.openPathLabel}
                  prepareFileLabel={labels.authGuidePrepareFile}
                  openDocsLabel={labels.authGuideOpenDocs}
                  bootstrappingLabel={labels.migrationHealthBootstrapping}
                  commandToastLabel={locale === "zh" ? `${report.tool_name} 认证命令` : `${report.tool_name} auth command`}
                  pathToastLabel={locale === "zh" ? `${report.tool_name} 路径` : `${report.tool_name} path`}
                  openPathToastLabel={locale === "zh" ? `${report.tool_name} 路径` : `${report.tool_name} path`}
                  docsToastLabel={locale === "zh" ? `${report.tool_name} 说明页` : `${report.tool_name} docs`}
                  onCopy={onCopy}
                  onOpen={onOpen}
                  onBootstrap={onBootstrapTool}
                />
              ))
            )}
          </div>
        </details>
      </div>
    </div>
  );
}

export default memo(SettingsMigrationCenterSectionComponent);
