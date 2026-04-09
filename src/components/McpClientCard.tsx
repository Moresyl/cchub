import { memo } from "react";
import { Monitor, Trash2 } from "lucide-react";

export interface McpClientCardClient {
  id: string;
  name: string;
  config_path: string;
  server_access: Record<string, boolean>;
  created_at: string | null;
}

interface McpClientCardProps {
  client: McpClientCardClient;
  selected: boolean;
  serverCountLabel: string;
  onSelect: (client: McpClientCardClient) => void;
  onDelete: (client: McpClientCardClient) => void;
}

function McpClientCardComponent({
  client,
  selected,
  serverCountLabel,
  onSelect,
  onDelete,
}: McpClientCardProps) {
  return (
    <div
      className={`card card-interactive ${selected ? "selected" : ""}`}
      style={{ padding: "16px 20px" }}
      onClick={() => onSelect(client)}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div className="icon-box" style={{ background: "var(--bg-elevated)", width: 36, height: 36, borderRadius: 6 }}>
            <Monitor size={16} style={{ color: "var(--text-secondary)" }} />
          </div>
          <div>
            <span style={{ fontSize: 13, fontWeight: 600 }}>{client.name}</span>
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
              {serverCountLabel}
            </p>
          </div>
        </div>
        <button
          className="btn btn-danger-ghost btn-icon-sm"
          onClick={(event) => {
            event.stopPropagation();
            onDelete(client);
          }}
        >
          <Trash2 size={15} />
        </button>
      </div>
    </div>
  );
}

export default memo(McpClientCardComponent);
