import { memo, type CSSProperties } from "react";

interface ToolsChoiceButtonProps {
  optionValue: string | number;
  label: string;
  active: boolean;
  onSelect: (optionValue: string | number) => void;
  style?: CSSProperties;
}

function ToolsChoiceButtonComponent({
  optionValue,
  label,
  active,
  onSelect,
  style,
}: ToolsChoiceButtonProps) {
  return (
    <button
      className={`btn btn-xs ${active ? "btn-primary" : "btn-secondary"}`}
      onClick={() => onSelect(optionValue)}
      style={style}
    >
      {label}
    </button>
  );
}

export default memo(ToolsChoiceButtonComponent);
