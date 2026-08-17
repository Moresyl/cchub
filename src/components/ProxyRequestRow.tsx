import { memo } from "react";

export interface ProxyRequestRowItem {
  request_id: string;
  provider_name: string;
  error_message: string | null;
  status_code: number;
}

interface ProxyRequestRowProps {
  item: ProxyRequestRowItem;
  success: boolean;
  toolLabel: string;
  modelLabel: string;
  costLabel: string;
  tokenLabel: string;
  latencyLabel: string;
  createdAtLabel: string;
  onSelect?: () => void;
}

function ProxyRequestRowComponent({
  item,
  success,
  toolLabel,
  modelLabel,
  costLabel,
  tokenLabel,
  latencyLabel,
  createdAtLabel,
  onSelect,
}: ProxyRequestRowProps) {
  return (
    <div
      className="list-row cv-auto"
      role={onSelect ? "button" : undefined}
      tabIndex={onSelect ? 0 : undefined}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (onSelect && (event.key === "Enter" || event.key === " ")) onSelect();
      }}
      style={{
        padding: "10px 12px",
        display: "flex",
        flexDirection: "column",
        alignItems: "stretch",
        gap: 6,
        containIntrinsicSize: "0 56px",
        cursor: onSelect ? "pointer" : undefined,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0, flex: 1 }}>
          <span className={`dot ${success ? "dot-active" : "dot-error"}`} />
          <span className="badge badge-muted" style={{ fontSize: 10 }}>
            {toolLabel}
          </span>
          <span
            style={{
              fontSize: 13,
              fontWeight: 500,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {item.provider_name}
          </span>
          <span
            style={{
              fontSize: 12,
              color: "var(--text-muted)",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {modelLabel}
          </span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12, flexShrink: 0 }}>
          <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
            {costLabel}
          </span>
          <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
            {tokenLabel}
          </span>
          <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
            {latencyLabel}
          </span>
          <span className={`badge ${success ? "badge-success" : "badge-danger"}`} style={{ fontSize: 10 }}>
            {item.status_code}
          </span>
        </div>
      </div>

      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
          {item.request_id}
        </span>
        <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{createdAtLabel}</span>
      </div>

      {item.error_message && (
        <div style={{ fontSize: 11, color: "var(--danger)", lineHeight: 1.5 }}>{item.error_message}</div>
      )}
    </div>
  );
}

export default memo(ProxyRequestRowComponent);
