import { memo } from "react";
import { Clock3, Copy, FileText, FolderOpen, Trash2 } from "lucide-react";

export interface SessionListItemSession {
  id: string;
  tool_id: string;
  tool_name: string;
  title: string;
  cwd: string | null;
  source_kind: string;
  source_backend: string;
  source_path: string;
  created_at: string | null;
  updated_at: string | null;
  preview: string;
  message_count: number;
  search_hit_count: number;
  can_resume: boolean;
  can_delete: boolean;
}

interface SessionListItemProps {
  session: SessionListItemSession;
  selected: boolean;
  query: string;
  resumeCommand: string | null;
  deleting: boolean;
  copyLabel: string;
  copyTitle: string;
  deleteTitle: string;
  deleteLabel: string;
  unknownTimeLabel: string;
  matchLabel: (count: number) => string;
  itemsLabel: (count: number) => string;
  onOpen: (session: SessionListItemSession) => void;
  onCopyResume: (command: string) => void;
  onDelete: (session: SessionListItemSession) => void;
}

function SessionListItemComponent({
  session,
  selected,
  query,
  resumeCommand,
  deleting,
  copyLabel,
  copyTitle,
  deleteTitle,
  deleteLabel,
  unknownTimeLabel,
  matchLabel,
  itemsLabel,
  onOpen,
  onCopyResume,
  onDelete,
}: SessionListItemProps) {
  return (
    <button
      className="card card-interactive"
      onClick={() => onOpen(session)}
      style={{
        textAlign: "left",
        padding: 14,
        borderColor: selected ? "var(--accent)" : "var(--border-default)",
        background: selected ? "var(--accent-subtle)" : "var(--bg-card)",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start" }}>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 8 }}>
            <span className="badge badge-accent" style={{ fontSize: 10 }}>{session.tool_name}</span>
            <span className="badge badge-muted" style={{ fontSize: 10 }}>{session.source_backend}</span>
            {session.search_hit_count > 0 && query.trim() && (
              <span className="badge badge-success" style={{ fontSize: 10 }}>
                {matchLabel(session.search_hit_count)}
              </span>
            )}
          </div>
          <div style={{ fontSize: 14, fontWeight: 700, color: "var(--text-primary)", lineHeight: 1.35 }}>
            {session.title}
          </div>
          <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 6, lineHeight: 1.5 }}>
            {session.preview}
          </div>
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap", marginTop: 10, fontSize: 11, color: "var(--text-muted)" }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
              <Clock3 size={12} />
              {session.updated_at || session.created_at || unknownTimeLabel}
            </span>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
              <FileText size={12} />
              {itemsLabel(session.message_count)}
            </span>
            {session.cwd && (
              <span style={{ display: "inline-flex", alignItems: "center", gap: 5, minWidth: 0 }}>
                <FolderOpen size={12} />
                <span style={{ maxWidth: 180, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {session.cwd}
                </span>
              </span>
            )}
          </div>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 8, flexShrink: 0 }}>
          {resumeCommand && (
            <button
              className="btn btn-secondary btn-xs"
              onClick={(event) => {
                event.stopPropagation();
                onCopyResume(resumeCommand);
              }}
              title={copyTitle}
            >
              <Copy size={12} />
              <span style={{ position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0, 0, 0, 0)" }}>{copyLabel}</span>
            </button>
          )}
          <button
            className="btn btn-danger btn-xs"
            onClick={(event) => {
              event.stopPropagation();
              onDelete(session);
            }}
            disabled={!session.can_delete || deleting}
            title={deleteTitle}
          >
            <Trash2 size={12} />
            <span style={{ position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0, 0, 0, 0)" }}>{deleteLabel}</span>
          </button>
        </div>
      </div>
    </button>
  );
}

export default memo(SessionListItemComponent);
