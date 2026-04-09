import { memo } from "react";
import ToolsToggleSwitch from "./ToolsToggleSwitch";

interface ToolsToggleCardProps {
  title: string;
  description: string;
  value: boolean;
  onChange: (value: boolean) => void;
  labelOn: string;
  labelOff: string;
}

function ToolsToggleCardComponent({
  title,
  description,
  value,
  onChange,
  labelOn,
  labelOff,
}: ToolsToggleCardProps) {
  return (
    <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <div>
        <h4 style={{ fontSize: 13, fontWeight: 700 }}>{title}</h4>
        <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{description}</p>
      </div>
      <ToolsToggleSwitch
        value={value}
        onChange={onChange}
        labelOn={labelOn}
        labelOff={labelOff}
      />
    </div>
  );
}

export default memo(ToolsToggleCardComponent);
