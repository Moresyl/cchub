import { memo } from "react";
import type { LucideIcon } from "lucide-react";

interface SettingsChoiceButtonProps<T extends string> {
  value: T;
  label: string;
  active: boolean;
  disabled?: boolean;
  icon?: LucideIcon;
  onSelect: (value: T) => void | Promise<void>;
}

function SettingsChoiceButtonComponent<T extends string>({
  value,
  label,
  active,
  disabled = false,
  icon: Icon,
  onSelect,
}: SettingsChoiceButtonProps<T>) {
  return (
    <button
      className={`btn btn-sm ${active ? "btn-primary" : "btn-secondary"}`}
      onClick={() => void onSelect(value)}
      disabled={disabled}
      style={{ gap: 6 }}
    >
      {Icon ? <Icon size={14} /> : null}
      {label}
    </button>
  );
}

export default memo(SettingsChoiceButtonComponent) as typeof SettingsChoiceButtonComponent;
