import { memo } from "react";
import { Edit3, Trash2, Zap } from "lucide-react";

export interface SkillCardSkill {
  id: string;
  name: string;
  description: string | null;
  tool_id: string | null;
  plugin_id: string | null;
  trigger_command: string | null;
  file_path: string | null;
}

interface SkillCardProps {
  skill: SkillCardSkill;
  selected: boolean;
  disabledLabel: string;
  editTitle: string;
  deleteTitle: string;
  enableTitle: string;
  disableTitle: string;
  onView: (skill: SkillCardSkill) => void;
  onToggle: (skill: SkillCardSkill) => void;
  onEdit: (skill: SkillCardSkill) => void;
  onDelete: (skill: SkillCardSkill) => void;
}

function SkillCardComponent({
  skill,
  selected,
  disabledLabel,
  editTitle,
  deleteTitle,
  enableTitle,
  disableTitle,
  onView,
  onToggle,
  onEdit,
  onDelete,
}: SkillCardProps) {
  const isDisabled = skill.file_path?.endsWith(".disabled") ?? false;

  return (
    <div
      className={`card card-interactive ${selected ? "selected" : ""}`}
      style={{ padding: "14px 18px", marginBottom: 6, opacity: isDisabled ? 0.5 : 1 }}
      onClick={() => onView(skill)}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12, minWidth: 0, flex: 1 }}>
          <div className="icon-box" style={{ background: "var(--warning-subtle)", width: 34, height: 34, borderRadius: 6 }}>
            <Zap size={15} style={{ color: "var(--warning)" }} />
          </div>
          <div style={{ minWidth: 0, flex: 1 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span style={{ fontSize: 13, fontWeight: 600, whiteSpace: "nowrap" }}>{skill.name}</span>
              {skill.plugin_id && <span className="badge badge-muted" style={{ fontSize: 10 }}>{skill.plugin_id}</span>}
              {isDisabled && (
                <span className="badge badge-muted" style={{ fontSize: 10 }}>{disabledLabel}</span>
              )}
            </div>
            {skill.description && (
              <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 2, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{skill.description}</p>
            )}
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
          {skill.trigger_command && (
            <code className="badge badge-accent" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 11 }}>{skill.trigger_command}</code>
          )}
          {skill.file_path && (
            <>
              <button
                onClick={(event) => {
                  event.stopPropagation();
                  onToggle(skill);
                }}
                title={isDisabled ? enableTitle : disableTitle}
                style={{
                  position: "relative",
                  width: 40,
                  height: 22,
                  borderRadius: 11,
                  border: "none",
                  cursor: "pointer",
                  background: isDisabled ? "var(--border-strong)" : "var(--success)",
                  transition: "background 0.2s",
                  padding: 0,
                  flexShrink: 0,
                }}
              >
                <span style={{
                  position: "absolute",
                  top: 2,
                  left: isDisabled ? 2 : 20,
                  width: 18,
                  height: 18,
                  borderRadius: "50%",
                  background: "#fff",
                  transition: "left 0.2s",
                  boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
                }} />
              </button>
              <button
                className="btn btn-ghost btn-icon-sm"
                onClick={(event) => {
                  event.stopPropagation();
                  onEdit(skill);
                }}
                title={editTitle}
              >
                <Edit3 size={13} />
              </button>
              <button
                className="btn btn-ghost btn-icon-sm"
                onClick={(event) => {
                  event.stopPropagation();
                  onDelete(skill);
                }}
                title={deleteTitle}
              >
                <Trash2 size={14} style={{ color: "var(--danger)" }} />
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default memo(SkillCardComponent);
