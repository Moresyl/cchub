import { memo, type ReactNode } from "react";
import { Switch } from "./ui/switch";

interface SettingsToggleRowProps {
  title: string;
  description: string;
  enabled: boolean;
  disabled?: boolean;
  titlePrefix?: ReactNode;
  onToggle: () => void | Promise<void>;
}

function SettingsToggleRowComponent({
  title,
  description,
  enabled,
  disabled = false,
  titlePrefix,
  onToggle,
}: SettingsToggleRowProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {titlePrefix}
          <p style={{ fontSize: 14, fontWeight: 500 }}>{title}</p>
        </div>
        <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{description}</p>
      </div>
      <Switch checked={enabled} onCheckedChange={() => void onToggle()} disabled={disabled} aria-label={title} />
    </div>
  );
}

export default memo(SettingsToggleRowComponent);
