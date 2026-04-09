import { memo } from "react";

interface SettingsLogLevelCardProps {
  level: string;
  description: string;
  active: boolean;
  disabled: boolean;
  onSelect: (level: string) => void | Promise<void>;
}

function SettingsLogLevelCardComponent({
  level,
  description,
  active,
  disabled,
  onSelect,
}: SettingsLogLevelCardProps) {
  return (
    <button
      type="button"
      className={`btn btn-sm ${active ? "btn-primary" : "btn-secondary"}`}
      onClick={() => void onSelect(level)}
      disabled={disabled}
      style={{ justifyContent: "space-between", alignItems: "flex-start", padding: "12px 14px", height: "auto", textAlign: "left" }}
    >
      <span>
        <span style={{ display: "block", fontFamily: "'JetBrains Mono', monospace", fontSize: 12, marginBottom: 6 }}>
          {level.toUpperCase()}
        </span>
        <span style={{ display: "block", fontSize: 12, color: active ? "inherit" : "var(--text-secondary)", lineHeight: 1.5 }}>
          {description}
        </span>
      </span>
    </button>
  );
}

export default memo(SettingsLogLevelCardComponent);
