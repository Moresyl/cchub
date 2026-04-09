import { memo } from "react";
import ToolsCheckboxOption from "./ToolsCheckboxOption";

interface ToolsCheckboxSectionOption {
  key: string;
  label: string;
  checked: boolean;
}

interface ToolsCheckboxSectionProps {
  title: string;
  options: ToolsCheckboxSectionOption[];
  onToggle: (optionKey: string, checked: boolean) => void;
}

function ToolsCheckboxSectionComponent({
  title,
  options,
  onToggle,
}: ToolsCheckboxSectionProps) {
  return (
    <div>
      <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>{title}</span>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "8px 16px", marginTop: 8 }}>
        {options.map((option) => (
          <ToolsCheckboxOption
            key={option.key}
            optionKey={option.key}
            label={option.label}
            checked={option.checked}
            onToggle={onToggle}
          />
        ))}
      </div>
    </div>
  );
}

export default memo(ToolsCheckboxSectionComponent);
