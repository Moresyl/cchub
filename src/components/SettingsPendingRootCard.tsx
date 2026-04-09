import { memo } from "react";
import { FolderOpen } from "lucide-react";

export interface SettingsPendingRootCardItem {
  project_root: string;
  file_count: number;
}

interface SettingsPendingRootCardProps {
  item: SettingsPendingRootCardItem;
  targetValue: string;
  oldPathLabel: string;
  newPathPlaceholder: string;
  pickLabel: string;
  applyLabel: string;
  applyingLabel: string;
  filesLabel: string;
  applying: boolean;
  onTargetChange: (sourcePath: string, nextValue: string) => void | Promise<void>;
  onPick: (sourcePath: string) => void | Promise<void>;
  onApply: (sourcePath: string, targetPath: string) => void | Promise<void>;
}

function SettingsPendingRootCardComponent({
  item,
  targetValue,
  oldPathLabel,
  newPathPlaceholder,
  pickLabel,
  applyLabel,
  applyingLabel,
  filesLabel,
  applying,
  onTargetChange,
  onPick,
  onApply,
}: SettingsPendingRootCardProps) {
  return (
    <div
      style={{
        padding: "12px 14px",
        borderRadius: 10,
        background: "var(--bg-card)",
        display: "flex",
        flexDirection: "column",
        gap: 10,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <div style={{ minWidth: 0 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{oldPathLabel}</div>
          <div style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>{item.project_root}</div>
        </div>
        <span className="badge badge-muted">{filesLabel}</span>
      </div>
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <input
          className="input"
          style={{ flex: 1, minWidth: 220, fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
          placeholder={newPathPlaceholder}
          value={targetValue}
          onChange={(event) => onTargetChange(item.project_root, event.target.value)}
        />
        <button
          className="btn btn-secondary btn-sm"
          type="button"
          onClick={() => onPick(item.project_root)}
        >
          <FolderOpen size={14} />
          {pickLabel}
        </button>
        <button
          className="btn btn-primary btn-sm"
          type="button"
          disabled={applying || !targetValue.trim()}
          onClick={() => onApply(item.project_root, targetValue)}
        >
          {applying ? applyingLabel : applyLabel}
        </button>
      </div>
    </div>
  );
}

export default memo(SettingsPendingRootCardComponent);
