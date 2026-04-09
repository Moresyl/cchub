import { memo } from "react";

export interface ActivityLogRowItem {
  id: number;
  server_name: string;
  request_type: string;
  status: string;
  latency_ms: number | null;
}

interface ActivityLogRowProps {
  item: ActivityLogRowItem;
  recordedAtLabel: string;
}

function ActivityLogRowComponent({ item, recordedAtLabel }: ActivityLogRowProps) {
  return (
    <div className="list-row" style={{ padding: "10px 12px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flex: 1, minWidth: 0 }}>
        <span className={`dot ${item.status === "success" ? "dot-active" : item.status === "error" ? "dot-error" : "dot-disabled"}`} />
        <span style={{ fontSize: 13, fontWeight: 500 }}>{item.server_name}</span>
        <span className="badge badge-muted" style={{ fontSize: 10 }}>
          {item.request_type}
        </span>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 12, flexShrink: 0 }}>
        {item.latency_ms != null && (
          <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
            {item.latency_ms}ms
          </span>
        )}
        <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
          {recordedAtLabel}
        </span>
      </div>
    </div>
  );
}

export default memo(ActivityLogRowComponent);
