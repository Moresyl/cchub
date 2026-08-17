import { lazy } from "react";
import { Check, Copy, Edit3, FileText, Share2 } from "lucide-react";
import type { I18n } from "../../lib/i18n";
import type { DetectedTool } from "../../types/skills";
import type { HealthCheckResult, McpServer } from "./helpers";

const CodeEditor = lazy(() => import("../../components/CodeEditor"));

interface McpServerDetailPanelProps {
  selected: McpServer;
  i: I18n;
  zh: boolean;
  copied: boolean;
  copyConfig: () => void;
  startEdit: (server: McpServer) => void;
  saveSuccess: boolean;
  healthResult?: HealthCheckResult;
  getSourceBadge: (source: string) => string;
  getSourceLabel: (source: string) => string;
  installedTools: DetectedTool[];
  toolSyncStatus: Record<string, boolean>;
  syncingTo: string | null;
  toggleToolSync: (toolId: string) => void;
}

export default function McpServerDetailPanel({
  selected,
  i,
  zh,
  copied,
  copyConfig,
  startEdit,
  saveSuccess,
  healthResult,
  getSourceBadge,
  getSourceLabel,
  installedTools,
  toolSyncStatus,
  syncingTo,
  toggleToolSync,
}: McpServerDetailPanelProps) {
  return (
    <div className="section-card" style={{ position: "sticky", top: 0 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 20 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <span
            className={`dot ${selected.status === "active" ? "dot-active" : selected.status === "error" ? "dot-error" : "dot-disabled"}`}
          />
          <h3 style={{ fontSize: 15, fontWeight: 700 }}>{selected.name}</h3>
          {selected.version && <span style={{ fontSize: 11, color: "var(--text-muted)" }}>v{selected.version}</span>}
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
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <span
            className={`badge ${selected.status === "active" ? "badge-success" : selected.status === "error" ? "badge-danger" : "badge-muted"}`}
          >
            {selected.status === "active" ? i.mcp.active : i.mcp.disabled}
          </span>
          <span className="badge badge-muted">{selected.transport}</span>
          <span className={`badge ${getSourceBadge(selected.source)}`}>{getSourceLabel(selected.source)}</span>
        </div>

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

        {healthResult && (
          <div>
            <span className="field-label">{i.mcp.healthStatus}</span>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                <span
                  className={`badge ${healthResult.status === "healthy" ? "badge-success" : healthResult.status === "unhealthy" ? "badge-danger" : "badge-muted"}`}
                >
                  {healthResult.status === "healthy"
                    ? i.mcp.healthy
                    : healthResult.status === "unhealthy"
                      ? i.mcp.unhealthy
                      : i.mcp.unknown}
                </span>
                <span className={`badge ${healthResult.command_exists ? "badge-success" : "badge-danger"}`}>
                  {i.mcp.commandExists}: {healthResult.command_exists ? "✓" : "✗"}
                </span>
                {healthResult.latency_ms != null && (
                  <span className="badge badge-muted">
                    {i.mcp.latency}: {healthResult.latency_ms}ms
                  </span>
                )}
              </div>
              {healthResult.error_message && (
                <div className="code-block" style={{ fontSize: 11, color: "var(--danger)" }}>
                  {healthResult.error_message}
                </div>
              )}
            </div>
          </div>
        )}

        <div>
          <span className="field-label">{i.mcp.command}</span>
          <div className="code-block" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}>
            {selected.command || i.common.na}
          </div>
        </div>
        <div>
          <span className="field-label">{i.mcp.arguments}</span>
          <CodeEditor value={formatJson(selected.args)} language="json" readOnly minHeight={80} maxHeight={180} />
        </div>
        <div>
          <span className="field-label">{i.mcp.environment}</span>
          <CodeEditor value={formatEnvironment(selected.env)} language="json" readOnly minHeight={80} maxHeight={180} />
        </div>

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
  );
}

function formatEnvironment(value: string) {
  try {
    const environment = JSON.parse(value);
    return Object.keys(environment).length ? JSON.stringify(environment, null, 2) : "{}";
  } catch {
    return value;
  }
}

function formatJson(value: string) {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}
