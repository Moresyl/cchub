import type { ReactNode } from "react";
import { Check, FolderOpen, PackagePlus, RefreshCw, Upload } from "lucide-react";

interface SkillsPageHeaderProps {
  title: string;
  subtitle: ReactNode;
  updateLabel: string;
  importLabel: string;
  installPluginLabel: string;
  exploreLabel: string;
  refreshLabel: string;
  batchUpdating: boolean;
  canBatchUpdate: boolean;
  onBatchUpdate: () => void;
  onImport: () => void;
  onInstallPlugin: () => void;
  onExplore: () => void;
  onRefresh: () => void;
}

export default function SkillsPageHeader({
  title,
  subtitle,
  updateLabel,
  importLabel,
  installPluginLabel,
  exploreLabel,
  refreshLabel,
  batchUpdating,
  canBatchUpdate,
  onBatchUpdate,
  onImport,
  onInstallPlugin,
  onExplore,
  onRefresh,
}: SkillsPageHeaderProps) {
  return (
    <div className="page-header">
      <div>
        <h2 className="page-title">{title}</h2>
        <p className="page-subtitle">{subtitle}</p>
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <button
          className="btn btn-secondary btn-sm"
          onClick={onBatchUpdate}
          disabled={!canBatchUpdate}
          style={{ gap: 6 }}
        >
          {batchUpdating ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Check size={14} />}
          {updateLabel}
        </button>
        <button className="btn btn-secondary btn-sm" onClick={onImport} style={{ gap: 6 }}>
          <Upload size={14} />
          {importLabel}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          onClick={onInstallPlugin}
          style={{ gap: 6 }}
          title={installPluginLabel}
        >
          <PackagePlus size={14} />
          {installPluginLabel}
        </button>
        <button className="btn btn-secondary btn-sm" onClick={onExplore} style={{ gap: 6 }}>
          <FolderOpen size={14} />
          {exploreLabel}
        </button>
        <button className="btn btn-secondary btn-sm" onClick={onRefresh}>
          <RefreshCw size={14} />
          {refreshLabel}
        </button>
      </div>
    </div>
  );
}
