import { memo } from "react";
import { ArrowRight, Download } from "lucide-react";

export interface UpdateCardInfo {
  item_type: string;
  item_id: string;
  item_name: string;
  current_version: string;
  latest_version: string;
}

interface UpdateCardProps {
  update: UpdateCardInfo;
  updateLabel: string;
}

function UpdateCardComponent({
  update,
  updateLabel,
}: UpdateCardProps) {
  return (
    <div
      className="card card-hover"
      style={{ padding: "20px 24px", display: "flex", alignItems: "center", justifyContent: "space-between" }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
        <div className="icon-box" style={{ background: "var(--warning-subtle)" }}>
          <Download size={17} style={{ color: "var(--warning)" }} />
        </div>
        <div>
          <span style={{ fontSize: 14, fontWeight: 600 }}>{update.item_name}</span>
          <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 6 }}>
            <span className="badge badge-muted" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 11 }}>{update.current_version}</span>
            <ArrowRight size={13} style={{ color: "var(--text-muted)" }} />
            <span className="badge badge-success" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 11 }}>{update.latest_version}</span>
          </div>
        </div>
      </div>
      <button className="btn btn-primary btn-sm">
        <Download size={14} />{updateLabel}
      </button>
    </div>
  );
}

export default memo(UpdateCardComponent);
