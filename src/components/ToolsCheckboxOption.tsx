import { memo } from "react";

interface ToolsCheckboxOptionProps {
  optionKey: string;
  label: string;
  checked: boolean;
  onToggle: (optionKey: string, checked: boolean) => void;
}

function ToolsCheckboxOptionComponent({
  optionKey,
  label,
  checked,
  onToggle,
}: ToolsCheckboxOptionProps) {
  return (
    <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, cursor: "pointer" }}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onToggle(optionKey, event.target.checked)}
      />
      {label}
    </label>
  );
}

export default memo(ToolsCheckboxOptionComponent);
