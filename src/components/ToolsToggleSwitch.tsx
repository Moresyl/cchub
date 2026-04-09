import { memo } from "react";

interface ToolsToggleSwitchProps {
  value: boolean;
  onChange: (value: boolean) => void;
  labelOn: string;
  labelOff: string;
}

function ToolsToggleSwitchComponent({
  value,
  onChange,
  labelOn,
  labelOff,
}: ToolsToggleSwitchProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
      <button className={`toggle toggle-sm ${value ? "on" : "off"}`} onClick={() => onChange(!value)}>
        <span className="toggle-knob" />
      </button>
      <span style={{ fontSize: 12, color: value ? "var(--success)" : "var(--text-muted)", fontWeight: 500 }}>
        {value ? labelOn : labelOff}
      </span>
    </div>
  );
}

export default memo(ToolsToggleSwitchComponent);
