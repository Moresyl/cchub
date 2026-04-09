import { memo } from "react";
import { CheckCircle, Download, Edit3, ExternalLink, Tag, Trash2 } from "lucide-react";

export interface MarketplaceSkillCardEntry {
  id: string;
  name: string;
  description: string;
  description_zh: string | null;
  category: string;
  author: string | null;
  github_url: string | null;
  cover_url: string | null;
  tags: string[];
  content: string;
  file_path?: string | null;
}

interface MarketplaceSkillCardProps {
  skill: MarketplaceSkillCardEntry;
  description: string;
  installed: boolean;
  installing: boolean;
  installedLabel: string;
  installLabel: string;
  installingLabel: string;
  editTitle: string;
  uninstallTitle: string;
  githubLabel: string;
  onPreview: (skill: MarketplaceSkillCardEntry) => void;
  onInstall: (skill: MarketplaceSkillCardEntry) => void;
  onEdit: (skill: MarketplaceSkillCardEntry) => void;
  onUninstall: (skill: MarketplaceSkillCardEntry) => void;
  onOpenGithub: (url: string) => void;
}

function MarketplaceSkillCardComponent({
  skill,
  description,
  installed,
  installing,
  installedLabel,
  installLabel,
  installingLabel,
  editTitle,
  uninstallTitle,
  githubLabel,
  onPreview,
  onInstall,
  onEdit,
  onUninstall,
  onOpenGithub,
}: MarketplaceSkillCardProps) {
  return (
    <div
      className="card card-hover"
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 10, cursor: "pointer" }}
      onClick={() => onPreview(skill)}
    >
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
          <span style={{ fontSize: 14, fontWeight: 600 }}>{skill.name}</span>
          <span className="badge badge-muted" style={{ fontSize: 10 }}>{skill.category}</span>
        </div>
        <p style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.5, display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden" }}>
          {description}
        </p>
      </div>
      {skill.tags.length > 0 && (
        <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
          {skill.tags.slice(0, 4).map((tag) => (
            <span key={tag} className="badge badge-muted" style={{ fontSize: 10, display: "flex", alignItems: "center", gap: 3 }}>
              <Tag size={9} />{tag}
            </span>
          ))}
        </div>
      )}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: "auto" }}>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          {skill.author && <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{skill.author}</span>}
          {skill.github_url && (
            <button
              className="badge badge-accent"
              style={{ fontSize: 10, display: "flex", alignItems: "center", gap: 3, cursor: "pointer", border: "none", background: "var(--accent-subtle)" }}
              onClick={(event) => {
                event.stopPropagation();
                onOpenGithub(skill.github_url!);
              }}
            >
              <ExternalLink size={10} />{githubLabel}
            </button>
          )}
        </div>
        {installed ? (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <button
              className="btn btn-ghost btn-icon-sm"
              onClick={(event) => {
                event.stopPropagation();
                onEdit(skill);
              }}
              title={editTitle}
            >
              <Edit3 size={13} style={{ color: "var(--text-muted)" }} />
            </button>
            <button
              className="btn btn-danger-ghost btn-icon-sm"
              onClick={(event) => {
                event.stopPropagation();
                onUninstall(skill);
              }}
              title={uninstallTitle}
            >
              <Trash2 size={13} />
            </button>
            <span className="badge badge-success" style={{ display: "flex", alignItems: "center", gap: 4 }}>
              <CheckCircle size={12} />{installedLabel}
            </span>
          </div>
        ) : (
          <button
            className="btn btn-primary btn-sm"
            onClick={(event) => {
              event.stopPropagation();
              onInstall(skill);
            }}
            disabled={installing}
          >
            <Download size={13} />{installing ? installingLabel : installLabel}
          </button>
        )}
      </div>
    </div>
  );
}

export default memo(MarketplaceSkillCardComponent);
