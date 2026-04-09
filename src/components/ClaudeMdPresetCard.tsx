import { memo } from "react";
import { Pencil, Trash2 } from "lucide-react";

interface PromptPresetCardItem {
  id: string;
  name: string;
  content: string;
  created_at: string;
  updated_at: string;
}

interface ClaudeMdPresetCardProps {
  preset: PromptPresetCardItem;
  isActive: boolean;
  activating: boolean;
  activeLabel: string;
  activationHint: string;
  editTitle: string;
  deleteTitle: string;
  syncLabel: string;
  resyncLabel: string;
  syncingLabel: string;
  onEdit: (preset: PromptPresetCardItem) => void;
  onDelete: (preset: PromptPresetCardItem) => void;
  onActivate: (presetId: string) => void | Promise<void>;
}

function ClaudeMdPresetCardComponent({
  preset,
  isActive,
  activating,
  activeLabel,
  activationHint,
  editTitle,
  deleteTitle,
  syncLabel,
  resyncLabel,
  syncingLabel,
  onEdit,
  onDelete,
  onActivate,
}: ClaudeMdPresetCardProps) {
  return (
    <div className="card" style={{ padding: "14px 16px", background: isActive ? "var(--bg-elevated)" : "var(--bg-card)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "flex-start", marginBottom: 10 }}>
        <div style={{ minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            <span style={{ fontSize: 14, fontWeight: 700 }}>{preset.name}</span>
            {isActive && <span className="badge badge-success" style={{ fontSize: 10 }}>{activeLabel}</span>}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
            {preset.updated_at.replace("T", " ").slice(0, 19)}
          </div>
        </div>
        <div style={{ display: "flex", gap: 4, flexShrink: 0 }}>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => onEdit(preset)} title={editTitle}>
            <Pencil size={14} />
          </button>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => onDelete(preset)} title={deleteTitle}>
            <Trash2 size={14} style={{ color: "var(--danger)" }} />
          </button>
        </div>
      </div>
      <div className="code-block" style={{ fontSize: 11, maxHeight: 120, overflow: "auto", marginBottom: 10 }}>
        {preset.content}
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
          {activationHint}
        </div>
        <button
          className={`btn btn-sm ${isActive ? "btn-secondary" : "btn-primary"}`}
          disabled={activating}
          onClick={() => onActivate(preset.id)}
        >
          {activating
            ? syncingLabel
            : isActive
              ? resyncLabel
              : syncLabel}
        </button>
      </div>
    </div>
  );
}

export default memo(ClaudeMdPresetCardComponent);
