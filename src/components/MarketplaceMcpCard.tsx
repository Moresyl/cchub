import { memo } from "react";
import { CheckCircle, Download, Edit3, ExternalLink, Key, Trash2 } from "lucide-react";

export interface MarketplaceMcpCardEntry {
  id: string;
  name: string;
  description: string;
  category: string;
  install_type: string;
  package_name: string | null;
  github_url: string | null;
  command: string;
  args: string[];
  env_keys: string[];
  source: string;
}

interface MarketplaceMcpCardProps {
  entry: MarketplaceMcpCardEntry;
  categoryLabel: string;
  installed: boolean;
  installing: boolean;
  installedLabel: string;
  installLabel: string;
  installingLabel: string;
  editTitle: string;
  uninstallTitle?: string;
  githubLabel: string;
  keysLabel: string;
  onPreview: (entry: MarketplaceMcpCardEntry) => void;
  onInstall: (entry: MarketplaceMcpCardEntry) => void;
  onEdit: (entry: MarketplaceMcpCardEntry) => void;
  onUninstall?: (entry: MarketplaceMcpCardEntry) => void;
  onOpenGithub: (url: string) => void;
}

function MarketplaceMcpCardComponent({
  entry,
  categoryLabel,
  installed,
  installing,
  installedLabel,
  installLabel,
  installingLabel,
  editTitle,
  uninstallTitle,
  githubLabel,
  keysLabel,
  onPreview,
  onInstall,
  onEdit,
  onUninstall,
  onOpenGithub,
}: MarketplaceMcpCardProps) {
  return (
    <div
      className="card card-hover"
      style={{ padding: "16px 20px", display: "flex", flexDirection: "column", gap: 12, cursor: "pointer" }}
      onClick={() => onPreview(entry)}
    >
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
          <span style={{ fontSize: 14, fontWeight: 600 }}>{entry.name}</span>
          <span className="badge badge-muted" style={{ fontSize: 10 }}>{categoryLabel}</span>
        </div>
        <p style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.5, display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden" }}>
          {entry.description}
        </p>
      </div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: "auto" }}>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          {entry.package_name && <span className="badge badge-muted" style={{ fontSize: 10 }}>{entry.install_type}</span>}
          {entry.env_keys.length > 0 && (
            <span className="badge badge-warning" style={{ fontSize: 10, display: "flex", alignItems: "center", gap: 3 }}>
              <Key size={10} />{entry.env_keys.length} {keysLabel}
            </span>
          )}
          {entry.github_url && (
            <button
              className="badge badge-accent"
              style={{ fontSize: 10, display: "flex", alignItems: "center", gap: 3, cursor: "pointer", border: "none", background: "var(--accent-subtle)" }}
              onClick={(event) => {
                event.stopPropagation();
                onOpenGithub(entry.github_url!);
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
                onEdit(entry);
              }}
              title={editTitle}
            >
              <Edit3 size={13} style={{ color: "var(--text-muted)" }} />
            </button>
            {onUninstall && (
              <button
                className="btn btn-ghost btn-icon-sm"
                onClick={(event) => {
                  event.stopPropagation();
                  onUninstall(entry);
                }}
                title={uninstallTitle ?? "Uninstall"}
              >
                <Trash2 size={13} style={{ color: "var(--danger)" }} />
              </button>
            )}
            <span className="badge badge-success" style={{ display: "flex", alignItems: "center", gap: 4 }}>
              <CheckCircle size={12} />{installedLabel}
            </span>
          </div>
        ) : (
          <button
            className="btn btn-primary btn-sm"
            onClick={(event) => {
              event.stopPropagation();
              onInstall(entry);
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

export default memo(MarketplaceMcpCardComponent);
