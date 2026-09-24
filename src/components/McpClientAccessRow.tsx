import { memo } from "react";
import { Switch } from "./ui/switch";

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
        <Switch
          checked={hasAccess}
          onCheckedChange={() => onToggle(server.id)}
          aria-label={`${server.name}: ${hasAccess ? allowedLabel : deniedLabel}`}
        />
      ) : (
        <span className={`badge ${hasAccess ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
          {hasAccess ? allowedLabel : deniedLabel}
        </span>
      )}
    </div>
  );
}

export default memo(McpClientAccessRowComponent);
