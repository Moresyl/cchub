import { memo } from "react";
import { Copy, FolderOpen } from "lucide-react";

export interface SettingsToolHealthCardReport {
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

interface SettingsToolHealthCardProps {
  report: SettingsToolHealthCardReport;
  installCommand: string | null;
  bootstrapping: boolean;
  cliMissingLabel: string;
  configDirMissingLabel: string;
  configMissingLabel: string;
  mcpMissingLabel: string;
  skillsMissingLabel: string;
  installActionLabel: string;
  bootstrapActionLabel: string;
  bootstrappingLabel: string;
  cliLabel: string;
  pathLabel: string;
  statusOkLabel: string;
  statusMissingLabel: string;
  configDirLabel: string;
  configFileLabel: string;
  mcpConfigLabel: string;
  skillsDirLabel: string;
  customPathLabel: string;
  installCommandToastLabel: string;
  onCopy: (value: string, label: string) => void | Promise<void>;
  onBootstrap: (toolId: string, toolName: string) => void | Promise<void>;
}

function SettingsToolHealthCardComponent({
  report,
  installCommand,
  bootstrapping,
  cliMissingLabel,
  configDirMissingLabel,
  configMissingLabel,
  mcpMissingLabel,
  skillsMissingLabel,
  installActionLabel,
  bootstrapActionLabel,
  bootstrappingLabel,
  cliLabel,
  pathLabel,
  statusOkLabel,
  statusMissingLabel,
  configDirLabel,
  configFileLabel,
  mcpConfigLabel,
  skillsDirLabel,
  customPathLabel,
  installCommandToastLabel,
  onCopy,
  onBootstrap,
}: SettingsToolHealthCardProps) {
  const issueBadges = [
    !report.cli_available ? cliMissingLabel : null,
    !report.config_dir_exists ? configDirMissingLabel : null,
    !report.config_exists ? configMissingLabel : null,
    !report.mcp_config_exists ? mcpMissingLabel : null,
    !report.skills_dir_exists ? skillsMissingLabel : null,
  ].filter(Boolean) as string[];

  const detailRows = [
    [configDirLabel, report.config_dir_exists, report.config_dir],
    [configFileLabel, report.config_exists, report.config_path],
    [mcpConfigLabel, report.mcp_config_exists, report.mcp_config_path],
    [skillsDirLabel, report.skills_dir_exists, report.skills_dir],
  ] as const;

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
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>{report.tool_name}</span>
          {issueBadges.map((badge) => (
            <span key={badge} className="badge badge-muted" style={{ fontSize: 10 }}>{badge}</span>
          ))}
        </div>
        {!report.cli_available && installCommand ? (
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => onCopy(installCommand, installCommandToastLabel)}
              style={{ gap: 6 }}
            >
              <Copy size={12} />
              {installActionLabel}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => onBootstrap(report.tool_id, report.tool_name)}
              disabled={bootstrapping}
              style={{ gap: 6 }}
            >
              <FolderOpen size={12} className={bootstrapping ? "spin" : ""} />
              {bootstrapping ? bootstrappingLabel : bootstrapActionLabel}
            </button>
          </div>
        ) : (
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => onBootstrap(report.tool_id, report.tool_name)}
            disabled={bootstrapping}
            style={{ gap: 6 }}
          >
            <FolderOpen size={12} className={bootstrapping ? "spin" : ""} />
            {bootstrapping ? bootstrappingLabel : bootstrapActionLabel}
          </button>
        )}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: 8 }}>
        <div style={{ fontSize: 12 }}>
          <div style={{ color: "var(--text-muted)", marginBottom: 4 }}>{cliLabel}</div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span className={`badge ${report.cli_available ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
              {report.cli_available ? statusOkLabel : statusMissingLabel}
            </span>
            <span style={{ fontFamily: "'JetBrains Mono', monospace" }}>{report.cli_command}</span>
          </div>
        </div>
        <div style={{ fontSize: 12 }}>
          <div style={{ color: "var(--text-muted)", marginBottom: 4 }}>{pathLabel}</div>
          <div style={{ fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all", color: "var(--text-secondary)" }}>
            {report.config_dir}
          </div>
          {(report.has_custom_config_dir || report.has_custom_mcp_config_path || report.has_custom_skills_dir) && (
            <div style={{ marginTop: 6 }}>
              <span className="badge badge-accent" style={{ fontSize: 10 }}>{customPathLabel}</span>
            </div>
          )}
        </div>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 8 }}>
        {detailRows.map(([label, ok, path]) => (
          <div key={`${report.tool_id}-${label}`} style={{ padding: "8px 10px", borderRadius: 8, background: "var(--bg-input)" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
              <span className={`badge ${ok ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
                {ok ? statusOkLabel : statusMissingLabel}
              </span>
              <span style={{ fontSize: 12 }}>{label}</span>
            </div>
            <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
              {path}
            </div>
          </div>
        ))}
      </div>
      {!report.cli_available && installCommand && (
        <code style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
          {installCommand}
        </code>
      )}
    </div>
  );
}

export default memo(SettingsToolHealthCardComponent);
