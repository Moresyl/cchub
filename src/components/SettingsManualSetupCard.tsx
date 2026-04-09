import { memo } from "react";
import { Copy, FolderOpen, Link2 } from "lucide-react";

export interface SettingsManualSetupCardReport {
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

interface SettingsManualSetupCardProps {
  report: SettingsManualSetupCardReport;
  description: string;
  installUrl: string | null;
  bootstrapping: boolean;
  copyCommandLabel: string;
  copyPathLabel: string;
  openPathLabel: string;
  prepareFileLabel: string;
  openDocsLabel: string;
  bootstrappingLabel: string;
  commandToastLabel: string;
  pathToastLabel: string;
  openPathToastLabel: string;
  docsToastLabel: string;
  onCopy: (value: string, label: string) => void | Promise<void>;
  onOpen: (target: string, label: string) => void | Promise<void>;
  onBootstrap: (toolId: string, toolName: string) => void | Promise<void>;
}

function SettingsManualSetupCardComponent({
  report,
  description,
  installUrl,
  bootstrapping,
  copyCommandLabel,
  copyPathLabel,
  openPathLabel,
  prepareFileLabel,
  openDocsLabel,
  bootstrappingLabel,
  commandToastLabel,
  pathToastLabel,
  openPathToastLabel,
  docsToastLabel,
  onCopy,
  onOpen,
  onBootstrap,
}: SettingsManualSetupCardProps) {
  return (
    <div
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
            onClick={() => onCopy(report.manual_setup_command || "", commandToastLabel)}
            style={{ gap: 6 }}
          >
            <Copy size={12} />
            {copyCommandLabel}
          </button>
        )}
        {report.manual_setup_path && (
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => onCopy(report.manual_setup_path || "", pathToastLabel)}
            style={{ gap: 6 }}
          >
            <Copy size={12} />
            {copyPathLabel}
          </button>
        )}
        {report.manual_setup_path && (
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => onOpen(report.manual_setup_path || "", openPathToastLabel)}
            style={{ gap: 6 }}
          >
            <FolderOpen size={12} />
            {openPathLabel}
          </button>
        )}
        <button
          className="btn btn-secondary btn-sm"
          onClick={() => onBootstrap(report.tool_id, report.tool_name)}
          disabled={bootstrapping}
          style={{ gap: 6 }}
        >
          <FolderOpen size={12} className={bootstrapping ? "spin" : ""} />
          {bootstrapping ? bootstrappingLabel : prepareFileLabel}
        </button>
        {installUrl && (
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => onOpen(installUrl, docsToastLabel)}
            style={{ gap: 6 }}
          >
            <Link2 size={12} />
            {openDocsLabel}
          </button>
        )}
      </div>
    </div>
  );
}

export default memo(SettingsManualSetupCardComponent);
