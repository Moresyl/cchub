import { memo } from "react";
import { FolderOpen } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

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
          <div style={{ fontSize: 12, fontFamily: "var(--font-code)", wordBreak: "break-all" }}>
            {item.project_root}
          </div>
        </div>
        <span className="badge badge-muted">{filesLabel}</span>
      </div>
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <Input
          className="flex-1 min-w-[min(220px,100%)] font-mono"
          aria-label={newPathPlaceholder}
          placeholder={newPathPlaceholder}
          value={targetValue}
          onChange={(event) => onTargetChange(item.project_root, event.target.value)}
        />
        <Button variant="outline" type="button" onClick={() => onPick(item.project_root)}>
          <FolderOpen size={14} />
          {pickLabel}
        </Button>
        <Button
          type="button"
          disabled={applying || !targetValue.trim()}
          onClick={() => onApply(item.project_root, targetValue)}
        >
          {applying ? applyingLabel : applyLabel}
        </Button>
      </div>
    </div>
  );
}

export default memo(SettingsPendingRootCardComponent);
