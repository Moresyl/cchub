import { memo } from "react";
import { Check, Edit3, FolderOpen, Monitor, Trash2, X } from "lucide-react";

export interface WorkspaceCardWorkspace {
  id: string;
  name: string;
  description: string | null;
  base_path: string | null;
  is_active: boolean;
  created_at: string | null;
}

interface WorkspaceCardProps {
  workspace: WorkspaceCardWorkspace;
  isEditing: boolean;
  editName: string;
  editDesc: string;
  editPath: string;
  activeLabel: string;
  editTitle: string;
  deleteTitle: string;
  cancelTitle: string;
  saveTitle: string;
  descriptionPlaceholder: string;
  pathPlaceholder: string;
  pickFolderTitle: string;
  onSwitch: (id: string) => void;
  onStartEdit: (workspace: WorkspaceCardWorkspace) => void;
  onDelete: (workspace: WorkspaceCardWorkspace) => void;
  onCancelEdit: () => void;
  onSaveEdit: (workspace: WorkspaceCardWorkspace) => void;
  onEditNameChange: (value: string) => void;
  onEditDescChange: (value: string) => void;
  onEditPathChange: (value: string) => void;
  onPickFolder: () => void;
}

function WorkspaceCardComponent({
  workspace,
  isEditing,
  editName,
  editDesc,
  editPath,
  activeLabel,
  editTitle,
  deleteTitle,
  cancelTitle,
  saveTitle,
  descriptionPlaceholder,
  pathPlaceholder,
  pickFolderTitle,
  onSwitch,
  onStartEdit,
  onDelete,
  onCancelEdit,
  onSaveEdit,
  onEditNameChange,
  onEditDescChange,
  onEditPathChange,
  onPickFolder,
}: WorkspaceCardProps) {
  return (
    <div
      className={`card ${workspace.is_active ? "" : "card-interactive"}`}
      role={workspace.is_active || isEditing ? undefined : "button"}
      tabIndex={workspace.is_active || isEditing ? undefined : 0}
      style={{
        padding: "18px 22px",
        border: workspace.is_active ? "1px solid var(--border-strong)" : undefined,
        background: workspace.is_active ? "var(--bg-card-hover)" : undefined,
      }}
      onClick={() => {
        if (!workspace.is_active && !isEditing) {
          onSwitch(workspace.id);
        }
      }}
      onKeyDown={(event) => {
        if (
          event.target === event.currentTarget &&
          !workspace.is_active &&
          !isEditing &&
          (event.key === "Enter" || event.key === " ")
        ) {
          event.preventDefault();
          onSwitch(workspace.id);
        }
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
          <div
            className="icon-box"
            style={{ background: "var(--bg-elevated)", width: 40, height: 40, borderRadius: 6 }}
          >
            <Monitor size={18} style={{ color: workspace.is_active ? "var(--text-primary)" : "var(--text-muted)" }} />
          </div>
          <div>
            {isEditing ? (
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                <input
                  className="input"
                  style={{ fontSize: 13, padding: "4px 8px" }}
                  value={editName}
                  onChange={(event) => onEditNameChange(event.target.value)}
                />
                <input
                  className="input"
                  style={{ fontSize: 12, padding: "4px 8px" }}
                  value={editDesc}
                  onChange={(event) => onEditDescChange(event.target.value)}
                  placeholder={descriptionPlaceholder}
                />
                <div style={{ display: "flex", gap: 6 }}>
                  <input
                    className="input"
                    style={{ fontSize: 12, padding: "4px 8px", fontFamily: "'JetBrains Mono', monospace" }}
                    value={editPath}
                    onChange={(event) => onEditPathChange(event.target.value)}
                    placeholder={pathPlaceholder}
                  />
                  <button
                    className="btn btn-secondary btn-icon-sm"
                    onClick={(event) => {
                      event.stopPropagation();
                      onPickFolder();
                    }}
                    title={pickFolderTitle}
                    aria-label={pickFolderTitle}
                  >
                    <FolderOpen size={14} />
                  </button>
                </div>
              </div>
            ) : (
              <>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{ fontSize: 14, fontWeight: 600 }}>{workspace.name}</span>
                  {workspace.is_active && (
                    <span className="badge badge-success" style={{ fontSize: 10 }}>
                      {activeLabel}
                    </span>
                  )}
                </div>
                {workspace.description && (
                  <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{workspace.description}</p>
                )}
                {workspace.base_path && (
                  <div style={{ display: "flex", alignItems: "center", gap: 4, marginTop: 4 }}>
                    <FolderOpen size={11} style={{ color: "var(--text-muted)" }} />
                    <span
                      style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}
                    >
                      {workspace.base_path}
                    </span>
                  </div>
                )}
              </>
            )}
          </div>
        </div>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          {isEditing ? (
            <>
              <button
                className="btn btn-ghost btn-icon-sm"
                aria-label={cancelTitle}
                title={cancelTitle}
                onClick={(event) => {
                  event.stopPropagation();
                  onCancelEdit();
                }}
              >
                <X size={14} />
              </button>
              <button
                className="btn btn-primary btn-icon-sm"
                aria-label={saveTitle}
                title={saveTitle}
                onClick={(event) => {
                  event.stopPropagation();
                  onSaveEdit(workspace);
                }}
              >
                <Check size={14} />
              </button>
            </>
          ) : (
            <>
              <button
                className="btn btn-ghost btn-icon-sm"
                onClick={(event) => {
                  event.stopPropagation();
                  onStartEdit(workspace);
                }}
                title={editTitle}
                aria-label={editTitle}
              >
                <Edit3 size={14} />
              </button>
              {!workspace.is_active && (
                <button
                  className="btn btn-danger-ghost btn-icon-sm"
                  onClick={(event) => {
                    event.stopPropagation();
                    onDelete(workspace);
                  }}
                  title={deleteTitle}
                  aria-label={deleteTitle}
                >
                  <Trash2 size={14} />
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default memo(WorkspaceCardComponent);
