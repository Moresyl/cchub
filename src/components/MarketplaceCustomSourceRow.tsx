import { memo } from "react";
import { Trash2 } from "lucide-react";

interface MarketplaceCustomSourceRowProps {
  index: number;
  url: string;
  count: number;
  countLabel: string;
  removeTitle: string;
  onRemove: (index: number) => void;
}

function MarketplaceCustomSourceRowComponent({
  index,
  url,
  count,
  countLabel,
  removeTitle,
  onRemove,
}: MarketplaceCustomSourceRowProps) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "10px 14px",
        borderRadius: 8,
        background: "var(--bg-input)",
        gap: 12,
      }}
    >
      <div style={{ minWidth: 0, flex: 1 }}>
        <div
          style={{
            fontSize: 12,
            fontFamily: "'JetBrains Mono', monospace",
            color: "var(--text-secondary)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {url}
        </div>
        <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
          {count} {countLabel}
        </span>
      </div>
      <button
        className="btn btn-danger-ghost btn-icon-sm"
        onClick={() => onRemove(index)}
        title={removeTitle}
      >
        <Trash2 size={13} />
      </button>
    </div>
  );
}

export default memo(MarketplaceCustomSourceRowComponent);
