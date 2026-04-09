import { memo } from "react";

export interface McpClientAccessRowServer {
  id: string;
  name: string;
  status: string;
}

interface McpClientAccessRowProps {
  server: McpClientAccessRowServer;
  hasAccess: boolean;
  editing: boolean;
  allowedLabel: string;
  deniedLabel: string;
  onToggle: (serverId: string) => void;
}

function McpClientAccessRowComponent({
  server,
  hasAccess,
  editing,
  allowedLabel,
  deniedLabel,
  onToggle,
}: McpClientAccessRowProps) {
  return (
    <div className="list-row" style={{ padding: "10px 12px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <span className={`dot ${server.status === "active" ? "dot-active" : "dot-disabled"}`} />
        <span style={{ fontSize: 13, fontWeight: 500 }}>{server.name}</span>
      </div>
      {editing ? (
        <button
          className={`toggle ${hasAccess ? "on" : "off"}`}
          onClick={() => onToggle(server.id)}
          style={{ width: 36, height: 20 }}
        >
          <div className="toggle-knob" style={{ width: 14, height: 14, top: 3 }} />
        </button>
      ) : (
        <span className={`badge ${hasAccess ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
          {hasAccess ? allowedLabel : deniedLabel}
        </span>
      )}
    </div>
  );
}

export default memo(McpClientAccessRowComponent);
