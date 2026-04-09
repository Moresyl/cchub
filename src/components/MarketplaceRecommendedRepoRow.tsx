import { memo } from "react";
import { Check, Download, ExternalLink } from "lucide-react";

interface MarketplaceRecommendedRepoRowProps {
  repoName: string;
  description: string;
  isLoaded: boolean;
  isLoading: boolean;
  openLabel: string;
  loadLabel: string;
  loadingLabel: string;
  loadedLabel: string;
  onOpen: (repoName: string) => void;
  onLoad: (repoName: string, branch: string) => void;
  branch: string;
}

function MarketplaceRecommendedRepoRowComponent({
  repoName,
  description,
  isLoaded,
  isLoading,
  openLabel,
  loadLabel,
  loadingLabel,
  loadedLabel,
  onOpen,
  onLoad,
  branch,
}: MarketplaceRecommendedRepoRowProps) {
  return (
    <div
      style={{ display: "flex", alignItems: "center", gap: 10, padding: "8px 12px", borderRadius: 8, background: "var(--bg-input)" }}
    >
      <ExternalLink size={12} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
      <div style={{ minWidth: 0, flex: 1 }}>
        <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>{repoName}</div>
        <div style={{ fontSize: 11, color: "var(--text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {description}
        </div>
      </div>
      <button
        className="btn btn-xs btn-ghost"
        style={{ flexShrink: 0, gap: 4 }}
        onClick={() => onOpen(repoName)}
        title={openLabel}
      >
        <ExternalLink size={10} />
      </button>
      <button
        className={`btn btn-xs ${isLoaded ? "btn-ghost" : "btn-primary"}`}
        style={{ flexShrink: 0, gap: 4 }}
        disabled={isLoading || isLoaded}
        onClick={() => onLoad(repoName, branch)}
      >
        {isLoading ? (
          <>
            <div className="spinner" style={{ width: 10, height: 10 }} />
            {loadingLabel}
          </>
        ) : isLoaded ? (
          <>
            <Check size={10} style={{ color: "var(--success)" }} />
            {loadedLabel}
          </>
        ) : (
          <>
            <Download size={10} />
            {loadLabel}
          </>
        )}
      </button>
    </div>
  );
}

export default memo(MarketplaceRecommendedRepoRowComponent);
