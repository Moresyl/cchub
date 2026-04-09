import { memo } from "react";
import { GitBranch, Pencil, Trash2 } from "lucide-react";

export interface WorkflowCardFile {
  path: string;
  tool_id: string;
  tool_name: string;
  name: string;
  file_name: string;
  size_bytes: number;
  modified_at: string | null;
  content_preview: string;
  disabled: boolean;
}

interface WorkflowCardProps {
  file: WorkflowCardFile;
  toolColor: string;
  disabledLabel: string;
  editLabel: string;
  deleteLabel: string;
  toggleTitle: string;
  toggling: boolean;
  formatSize: (bytes: number) => string;
  onEdit: (file: WorkflowCardFile) => void;
  onToggle: (file: WorkflowCardFile) => void;
  onDelete: (file: WorkflowCardFile) => void;
}

function WorkflowCardComponent({
  file,
  toolColor,
  disabledLabel,
  editLabel,
  deleteLabel,
  toggleTitle,
  toggling,
  formatSize,
  onEdit,
  onToggle,
  onDelete,
}: WorkflowCardProps) {
  return (
    <div
      className="card"
      style={{ padding: "16px 20px", opacity: file.disabled ? 0.55 : 1, transition: "opacity 0.2s" }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <div className="icon-box" style={{ background: "var(--bg-elevated)", width: 36, height: 36, borderRadius: 6 }}>
          <GitBranch size={16} style={{ color: file.disabled ? "var(--text-muted)" : toolColor }} />
        </div>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <span style={{ fontSize: 13, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {file.name}
            </span>
            <span style={{
              fontSize: 10, padding: "1px 6px", borderRadius: 4, fontWeight: 600,
              background: `${toolColor}15`,
              color: toolColor,
            }}>{file.tool_name}</span>
            {file.disabled && (
              <span className="badge badge-muted" style={{ fontSize: 10, padding: "1px 6px" }}>
                {disabledLabel}
              </span>
            )}
            <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{formatSize(file.size_bytes)}</span>
            {file.modified_at && (
              <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{file.modified_at}</span>
            )}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 3, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {file.path}
          </div>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 10, flexShrink: 0 }}>
          <button className="btn btn-secondary btn-sm" onClick={() => onEdit(file)}>
            <Pencil size={13} />{editLabel}
          </button>

          <button
            onClick={() => onToggle(file)}
            disabled={toggling}
            title={toggleTitle}
            style={{
              position: "relative", width: 40, height: 22, borderRadius: 11,
              border: "none", padding: 0, flexShrink: 0,
              cursor: toggling ? "wait" : "pointer",
              background: file.disabled ? "var(--border-strong)" : "var(--success)",
              transition: "background 0.2s",
            }}
          >
            <span style={{
              position: "absolute", top: 2,
              left: file.disabled ? 2 : 20,
              width: 18, height: 18, borderRadius: "50%",
              background: "#fff", transition: "left 0.2s",
              boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
            }} />
          </button>

          <button
            className="btn btn-ghost btn-icon-sm"
            title={deleteLabel}
            onClick={() => onDelete(file)}
          >
            <Trash2 size={14} style={{ color: "var(--danger)" }} />
          </button>
        </div>
      </div>
    </div>
  );
}

export default memo(WorkflowCardComponent);
