import { memo } from "react";
import { Switch } from "./ui/switch";

interface ToolsToggleSwitchProps {
  value: boolean;
  onChange: (value: boolean) => void;
  labelOn: string;
  labelOff: string;
}

function ToolsToggleSwitchComponent({ value, onChange, labelOn, labelOff }: ToolsToggleSwitchProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
      <Switch checked={value} onCheckedChange={onChange} aria-label={value ? labelOn : labelOff} />
      <span style={{ fontSize: 12, color: value ? "var(--success)" : "var(--text-muted)", fontWeight: 500 }}>
        {value ? labelOn : labelOff}
      </span>
    </div>
  );
}

export default memo(ToolsToggleSwitchComponent);
