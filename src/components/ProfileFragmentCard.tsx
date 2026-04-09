import { memo } from "react";
import { ArrowRightLeft, Trash2 } from "lucide-react";

interface ProfileFragmentCardProps {
  fragmentId: string;
  name: string;
  targetTools: string[];
  toolNameMap: Record<string, string>;
  currentToolCompatible: boolean;
  compatibilityReadyLabel: string;
  compatibilityCommonOnlyLabel: string;
  updatedLabel: string;
  updatedAt: string;
  applyLabel: string;
  deleteTitle: string;
  deleting: boolean;
  onApply: (fragmentId: string) => void | Promise<void>;
  onDelete: (fragmentId: string) => void | Promise<void>;
}

function ProfileFragmentCardComponent({
  fragmentId,
  name,
  targetTools,
  toolNameMap,
  currentToolCompatible,
  compatibilityReadyLabel,
  compatibilityCommonOnlyLabel,
  updatedLabel,
  updatedAt,
  applyLabel,
  deleteTitle,
  deleting,
  onApply,
  onDelete,
}: ProfileFragmentCardProps) {
  return (
    <div
      className="card"
      style={{ padding: 12, background: "var(--bg-elevated)", display: "flex", gap: 12, justifyContent: "space-between", alignItems: "flex-start" }}
    >
      <div style={{ minWidth: 0, flex: 1 }}>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
          <span style={{ fontSize: 14, fontWeight: 600 }}>{name}</span>
          {targetTools.map((toolId) => (
            <span key={`${fragmentId}-${toolId}`} className="badge badge-muted" style={{ fontSize: 10 }}>
              {toolNameMap[toolId] || toolId}
            </span>
          ))}
          <span className={`badge ${currentToolCompatible ? "badge-success" : "badge-warning"}`} style={{ fontSize: 10 }}>
            {currentToolCompatible ? compatibilityReadyLabel : compatibilityCommonOnlyLabel}
          </span>
        </div>
        <div style={{ marginTop: 6, fontSize: 12, color: "var(--text-muted)" }}>
          {updatedLabel}: {updatedAt}
        </div>
      </div>
      <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
        <button className="btn btn-secondary btn-sm" type="button" onClick={() => onApply(fragmentId)} style={{ gap: 6 }}>
          <ArrowRightLeft size={14} />
          {applyLabel}
        </button>
        <button
          className="btn btn-danger-ghost btn-icon-sm"
          type="button"
          onClick={() => onDelete(fragmentId)}
          disabled={deleting}
          title={deleteTitle}
        >
          {deleting ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Trash2 size={14} />}
        </button>
      </div>
    </div>
  );
}

export default memo(ProfileFragmentCardComponent);
