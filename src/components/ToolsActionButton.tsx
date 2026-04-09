import { memo } from "react";
import type { LucideIcon } from "lucide-react";

interface ToolsActionButtonProps {
  label: string;
  icon: LucideIcon;
  pending: boolean;
  onClick: () => void;
  disabled?: boolean;
  variant?: "primary" | "secondary";
  title?: string;
  gap?: number;
  spinnerSize?: number;
  iconSize?: number;
}

function ToolsActionButtonComponent({
  label,
  icon: Icon,
  pending,
  onClick,
  disabled = false,
  variant = "secondary",
  title,
  gap = 4,
  spinnerSize = 12,
  iconSize = 12,
}: ToolsActionButtonProps) {
  return (
    <button
      className={`btn btn-${variant} btn-xs`}
      onClick={onClick}
      disabled={disabled}
      title={title}
      style={{ gap }}
    >
      {pending ? <div className="spinner" style={{ width: spinnerSize, height: spinnerSize }} /> : <Icon size={iconSize} />}
      {label}
    </button>
  );
}

export default memo(ToolsActionButtonComponent);
