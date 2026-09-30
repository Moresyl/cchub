import { memo } from "react";
import { FolderOpen } from "lucide-react";
import SettingsSummaryMetricCard from "./SettingsSummaryMetricCard";
import SettingsSummaryStatRow from "./SettingsSummaryStatRow";
import { Button } from "./ui/button";

export interface SettingsImportSummaryPanelSummary {
  imported_at: string;
  db_rows_restored: number;
  tool_configs_restored: number;
  skills_restored: number;
  full_files_restored: number;
  pending_project_files: number;
  safety_backup_path: string;
}

export interface SettingsImportSummaryPanelRescan {
  mcp_servers: number;
  skills: number;
  hooks: number;
  instruction_files: number;
  workflows: number;
  config_roots: number;
}

interface SettingsImportSummaryPanelProps {
  summary: SettingsImportSummaryPanelSummary | null;
  rescan: SettingsImportSummaryPanelRescan | null;
  emptyLabel: string;
  backupLabel: string;
  openPathLabel: string;
  rescanTitle: string;
  rescanEmptyLabel: string;
  importedAtLabel: string;
  dataLabel: string;
  toolConfigsLabel: string;
  skillsLabel: string;
  filesLabel: string;
  pendingLabel: string;
  mcpLabel: string;
  rescanSkillsLabel: string;
  hooksLabel: string;
  docsLabel: string;
  workflowsLabel: string;
  configRootsLabel: string;
  onOpenBackupPath: () => void | Promise<void>;
}

function SettingsImportSummaryPanelComponent({
  summary,
  rescan,
  emptyLabel,
  backupLabel,
  openPathLabel,
  rescanTitle,
  rescanEmptyLabel,
  importedAtLabel,
  dataLabel,
  toolConfigsLabel,
  skillsLabel,
  filesLabel,
  pendingLabel,
  mcpLabel,
  rescanSkillsLabel,
  hooksLabel,
  docsLabel,
  workflowsLabel,
  configRootsLabel,
  onOpenBackupPath,
}: SettingsImportSummaryPanelProps) {
  const importedDate = summary ? new Date(summary.imported_at) : null;
  const importedAt =
    importedDate && !Number.isNaN(importedDate.getTime())
      ? importedDate.toLocaleString(undefined, {
          year: "numeric",
          month: "2-digit",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit",
          hour12: false,
        })
      : (summary?.imported_at ?? "");
  return (
    <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: 12 }}>
      {summary ? (
        <>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))", gap: 8 }}>
            {[
              [importedAtLabel, importedAt],
              [dataLabel, String(summary.db_rows_restored)],
              [toolConfigsLabel, String(summary.tool_configs_restored)],
              [skillsLabel, String(summary.skills_restored)],
              [filesLabel, String(summary.full_files_restored)],
              [pendingLabel, String(summary.pending_project_files)],
            ].map(([label, value]) => (
              <SettingsSummaryMetricCard key={String(label)} label={String(label)} value={String(value)} />
            ))}
          </div>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 12,
              alignItems: "center",
              flexWrap: "wrap",
            }}
          >
            <div style={{ minWidth: 0 }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{backupLabel}</div>
              <div
                style={{
                  fontSize: 12,
                  fontFamily: "var(--font-code)",
                  color: "var(--text-secondary)",
                  wordBreak: "break-all",
                }}
              >
                {summary.safety_backup_path}
              </div>
            </div>
            <Button variant="outline" type="button" onClick={onOpenBackupPath}>
              <FolderOpen size={14} />
              {openPathLabel}
            </Button>
          </div>
        </>
      ) : (
        <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{emptyLabel}</div>
      )}

      <div style={{ paddingTop: 4 }}>
        <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 6 }}>{rescanTitle}</div>
        {rescan ? (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(120px, 1fr))", gap: 8 }}>
            {[
              [mcpLabel, rescan.mcp_servers],
              [rescanSkillsLabel, rescan.skills],
              [hooksLabel, rescan.hooks],
              [docsLabel, rescan.instruction_files],
              [workflowsLabel, rescan.workflows],
              [configRootsLabel, rescan.config_roots],
            ].map(([label, value]) => (
              <SettingsSummaryStatRow key={String(label)} label={String(label)} value={value} />
            ))}
          </div>
        ) : (
          <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{rescanEmptyLabel}</div>
        )}
      </div>
    </div>
  );
}

export default memo(SettingsImportSummaryPanelComponent);
