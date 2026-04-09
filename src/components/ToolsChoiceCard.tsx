import { memo, type CSSProperties } from "react";
import ToolsChoiceButton from "./ToolsChoiceButton";

interface ToolsChoiceCardOption {
  value: string | number;
  label: string;
  style?: CSSProperties;
}

interface ToolsChoiceCardProps {
  title: string;
  description: string;
  value: string | number;
  onSelect: (value: string | number) => void;
  options: ToolsChoiceCardOption[];
}

function ToolsChoiceCardComponent({
  title,
  description,
  value,
  onSelect,
  options,
}: ToolsChoiceCardProps) {
  return (
    <div className="card" style={{ padding: "14px 18px", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <div>
        <h4 style={{ fontSize: 13, fontWeight: 700 }}>{title}</h4>
        <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{description}</p>
      </div>
      <div style={{ display: "flex", gap: 6 }}>
        {options.map((option) => (
          <ToolsChoiceButton
            key={String(option.value)}
            optionValue={option.value}
            label={option.label}
            active={value === option.value}
            onSelect={onSelect}
            style={option.style}
          />
        ))}
      </div>
    </div>
  );
}

export default memo(ToolsChoiceCardComponent);
