import { memo } from "react";
import { CheckCircle, Download, ExternalLink, Loader2, Package } from "lucide-react";

export interface FeaturedSkillBundle {
  /** Stable identifier used for installing-state tracking */
  id: string;
  /** Display name (e.g. "Superpowers") */
  title: string;
  /** GitHub owner */
  owner: string;
  /** GitHub repo name */
  repo: string;
  /** Branch to fetch from */
  branch: string;
  /** Total expected skill count, shown in title chip */
  totalSkills: number;
  /** GitHub stars label, shown next to skill count (optional) */
  starsLabel?: string;
  /** Short tagline */
  description: string;
  /** Tag pills (skill names or workflow phases) */
  tags: string[];
  /** Full GitHub URL for the "View source" affordance */
  githubUrl: string;
}

export interface FeaturedSkillBundleCardProps {
  bundle: FeaturedSkillBundle;
  /** All bundle skills already installed locally */
  fullyInstalled: boolean;
  /** Currently installing — show progress */
  installing: boolean;
  /** Installed skill count (for "x / total" progress) */
  installedCount: number;
  installAllLabel: string;
  installingLabel: string;
  installedLabel: string;
  reinstallLabel: string;
  bundleBadgeLabel: string;
  githubLabel: string;
  onInstallAll: (bundle: FeaturedSkillBundle) => void;
  onOpenGithub: (url: string) => void;
}

function FeaturedSkillBundleCardComponent({
  bundle,
  fullyInstalled,
  installing,
  installedCount,
  installAllLabel,
  installingLabel,
  installedLabel,
  reinstallLabel,
  bundleBadgeLabel,
  githubLabel,
  onInstallAll,
  onOpenGithub,
}: FeaturedSkillBundleCardProps) {
  const buttonLabel = installing
    ? `${installingLabel} ${installedCount}/${bundle.totalSkills}`
    : fullyInstalled
      ? reinstallLabel
      : installAllLabel;

  return (
    <div
      className="card"
      style={{
        padding: 18,
        background: "linear-gradient(135deg, var(--bg-card) 0%, var(--bg-elevated) 100%)",
        border: "1px solid var(--border-default)",
        position: "relative",
        overflow: "hidden",
      }}
    >
      {/* Featured ribbon */}
      <div
        style={{
          position: "absolute",
          top: 0,
          right: 0,
          padding: "3px 10px",
          fontSize: 9,
          fontWeight: 700,
          letterSpacing: "0.08em",
          textTransform: "uppercase",
          background: "var(--text-primary)",
          color: "var(--bg-app)",
          borderBottomLeftRadius: 6,
        }}
      >
        {bundleBadgeLabel}
      </div>

      <div style={{ display: "flex", alignItems: "flex-start", gap: 12, marginBottom: 12 }}>
        <div
          className="icon-box"
          style={{
            background: "var(--bg-app)",
            width: 40,
            height: 40,
            borderRadius: 8,
            flexShrink: 0,
          }}
        >
          <Package size={20} style={{ color: "var(--text-primary)" }} />
        </div>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ display: "flex", alignItems: "baseline", gap: 8, flexWrap: "wrap" }}>
            <span style={{ fontSize: 15, fontWeight: 700, color: "var(--text-primary)" }}>
              {bundle.title}
            </span>
            <span style={{ fontSize: 11, color: "var(--text-muted)", fontWeight: 500 }}>
              {bundle.totalSkills} skills{bundle.starsLabel ? ` · ${bundle.starsLabel}` : ""}
            </span>
          </div>
          <p
            style={{
              fontSize: 12,
              color: "var(--text-secondary)",
              marginTop: 4,
              lineHeight: 1.5,
            }}
          >
            {bundle.description}
          </p>
        </div>
      </div>

      {bundle.tags.length > 0 && (
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: 4,
            marginBottom: 14,
          }}
        >
          {bundle.tags.slice(0, 8).map((tag) => (
            <span
              key={tag}
              style={{
                fontSize: 10,
                padding: "2px 7px",
                borderRadius: 10,
                background: "var(--bg-app)",
                color: "var(--text-secondary)",
                border: "1px solid var(--border-subtle)",
              }}
            >
              {tag}
            </span>
          ))}
          {bundle.tags.length > 8 && (
            <span
              style={{
                fontSize: 10,
                padding: "2px 7px",
                color: "var(--text-muted)",
              }}
            >
              +{bundle.tags.length - 8}
            </span>
          )}
        </div>
      )}

      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <button
          className={fullyInstalled && !installing ? "btn btn-secondary btn-sm" : "btn btn-primary btn-sm"}
          onClick={() => onInstallAll(bundle)}
          disabled={installing}
          style={{ flex: 1, gap: 6, justifyContent: "center" }}
        >
          {installing ? (
            <Loader2 size={13} className="spin" />
          ) : fullyInstalled ? (
            <CheckCircle size={13} />
          ) : (
            <Download size={13} />
          )}
          {fullyInstalled && !installing ? `${installedLabel} (${installedCount}/${bundle.totalSkills})` : buttonLabel}
        </button>
        <button
          className="btn btn-ghost btn-sm"
          onClick={() => onOpenGithub(bundle.githubUrl)}
          title={githubLabel}
          style={{ gap: 4 }}
        >
          <ExternalLink size={12} />
        </button>
      </div>
    </div>
  );
}

const FeaturedSkillBundleCard = memo(FeaturedSkillBundleCardComponent);
export default FeaturedSkillBundleCard;
