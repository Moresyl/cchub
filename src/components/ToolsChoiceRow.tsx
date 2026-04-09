import { memo, type CSSProperties } from "react";
import ToolsChoiceButton from "./ToolsChoiceButton";

interface ToolsChoiceRowOption {
  value: string | number;
  label: string;
  style?: CSSProperties;
}

interface ToolsChoiceRowProps {
  title: string;
  value: string | number;
  onSelect: (value: string | number) => void;
  options: ToolsChoiceRowOption[];
}

function ToolsChoiceRowComponent({
  title,
  value,
  onSelect,
  options,
}: ToolsChoiceRowProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <div>
        <span style={{ fontSize: 12, fontWeight: 600 }}>{title}</span>
      </div>
      <div style={{ display: "flex", gap: 4 }}>
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

export default memo(ToolsChoiceRowComponent);
