import { memo } from "react";

interface ToolsPermLevelLabelProps {
  level: number;
  label: string;
  color: string;
  active: boolean;
  onSelect: (value: string | number) => void;
}

function ToolsPermLevelLabelComponent({
  level,
  label,
  color,
  active,
  onSelect,
}: ToolsPermLevelLabelProps) {
  return (
    <span
      style={{ fontSize: 10, color: active ? color : "var(--text-muted)", fontWeight: active ? 700 : 400, cursor: "pointer" }}
      onClick={() => onSelect(level)}
    >
      {label}
    </span>
  );
}

export default memo(ToolsPermLevelLabelComponent);
