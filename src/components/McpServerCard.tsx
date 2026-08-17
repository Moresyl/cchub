import { memo } from "react";
import { Edit3, Trash2 } from "lucide-react";

export interface McpServerCardServer {
  id: string;
  name: string;
  command: string | null;
  args: string;
  env: string;
  status: string;
  transport: string;
  source: string;
  package_name: string | null;
  version: string | null;
  config_path: string | null;
}

interface McpServerCardProps {
  server: McpServerCardServer;
  selected: boolean;
  sourceBadge: string;
  sourceLabel: string;
  healthStatus: string | null;
  healthTitle: string | null;
  editTitle: string;
  deleteTitle: string;
  onSelect: (server: McpServerCardServer) => void;
  onEdit: (server: McpServerCardServer) => void;
  onDelete: (server: McpServerCardServer) => void;
}

function getHealthColor(status: string | null) {
  if (status === "healthy") return "var(--success)";
  if (status === "unhealthy") return "var(--danger)";
  return "var(--text-muted)";
}

function getCommandPreview(server: McpServerCardServer) {
  if (!server.command) return null;

  try {
    const parsedArgs = JSON.parse(server.args);
    if (Array.isArray(parsedArgs)) {
      return `${server.command} ${parsedArgs.join(" ")}`.trim();
    }
  } catch {
    // Ignore malformed args and fall back to command-only preview.
  }

  return server.command;
}

function McpServerCardComponent({
  server,
  selected,
  sourceBadge,
  sourceLabel,
  healthStatus,
  healthTitle,
  editTitle,
  deleteTitle,
  onSelect,
  onEdit,
  onDelete,
}: McpServerCardProps) {
  const commandPreview = getCommandPreview(server);

  return (
    <div
      className={`card card-interactive ${selected ? "selected" : ""}`}
      style={{ padding: "16px 20px", opacity: server.status === "disabled" ? 0.5 : 1 }}
      onClick={() => onSelect(server)}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span
            className={`dot ${server.status === "active" ? "dot-active" : server.status === "error" ? "dot-error" : "dot-disabled"}`}
          />
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span style={{ fontSize: 13, fontWeight: 600 }}>{server.name}</span>
              {server.version && <span style={{ fontSize: 11, color: "var(--text-muted)" }}>v{server.version}</span>}
              {healthStatus && (
                <span
                  title={healthTitle ?? undefined}
                  style={{
                    width: 8,
                    height: 8,
                    borderRadius: "50%",
                    background: getHealthColor(healthStatus),
                    display: "inline-block",
                  }}
                />
              )}
            </div>
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span className={`badge ${sourceBadge}`}>{sourceLabel}</span>
          {server.transport === "stdio" && (
            <button
              className="btn btn-ghost btn-icon-sm"
              onClick={(event) => {
                event.stopPropagation();
                onEdit(server);
              }}
              title={editTitle}
            >
              <Edit3 size={15} />
            </button>
          )}
          <button
            className="btn btn-danger-ghost btn-icon-sm"
            onClick={(event) => {
              event.stopPropagation();
              onDelete(server);
            }}
            title={deleteTitle}
          >
            <Trash2 size={15} />
          </button>
        </div>
      </div>
      {commandPreview && (
        <p
          style={{
            fontSize: 12,
            fontFamily: "'JetBrains Mono', monospace",
            color: "var(--text-muted)",
            marginTop: 8,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {commandPreview}
        </p>
      )}
    </div>
  );
}

export default memo(McpServerCardComponent);
