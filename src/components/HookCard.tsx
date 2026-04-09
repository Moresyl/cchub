import { memo } from "react";
import { Edit3, Trash2 } from "lucide-react";

export interface HookCardHook {
  id: string;
  event: string;
  matcher: string | null;
  command: string;
  scope: string;
  project_path: string | null;
  source_event: string | null;
  source_index: number | null;
  enabled: boolean;
  timeout: number | null;
}

interface HookCardProps {
  hook: HookCardHook;
  matcherLabel: string;
  timeoutLabel: string;
  globalLabel: string;
  projectLabel: string;
  editTitle: string;
  deleteTitle: string;
  onEdit: (hook: HookCardHook) => void;
  onDelete: (hook: HookCardHook) => void;
}

function HookCardComponent({
  hook,
  matcherLabel,
  timeoutLabel,
  globalLabel,
  projectLabel,
  editTitle,
  deleteTitle,
  onEdit,
  onDelete,
}: HookCardProps) {
  return (
    <div className="card card-hover" style={{ padding: "20px 24px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
          <span className="dot dot-active" />
          <span className="badge badge-accent" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 11 }}>{hook.event}</span>
          {hook.matcher && (
            <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {matcherLabel}: <span style={{ fontFamily: "'JetBrains Mono', monospace" }}>{hook.matcher}</span>
            </span>
          )}
          {hook.timeout && (
            <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
              {timeoutLabel}: {hook.timeout}ms
            </span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span className="badge badge-muted">{hook.scope === "global" ? globalLabel : projectLabel}</span>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => onEdit(hook)} title={editTitle}>
            <Edit3 size={14} />
          </button>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => onDelete(hook)}
            title={deleteTitle}
            style={{ color: "var(--danger)" }}
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>
      <div className="code-block" style={{ marginTop: 14 }}>{hook.command}</div>
      {hook.project_path && (
        <p
          style={{
            fontSize: 11,
            fontFamily: "'JetBrains Mono', monospace",
            color: "var(--text-muted)",
            marginTop: 10,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {hook.project_path}
        </p>
      )}
    </div>
  );
}

export default memo(HookCardComponent);
