import { memo } from "react";
import type { LucideIcon } from "lucide-react";
import { Button } from "./ui/button";

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
    <Button
      type="button"
      variant={active ? "default" : "secondary"}
      size="sm"
      aria-pressed={active}
      onClick={() => void onSelect(value)}
      disabled={disabled}
      className="gap-1.5"
    >
      {Icon ? <Icon size={14} /> : null}
      {label}
    </Button>
  );
}

export default memo(SettingsChoiceButtonComponent) as typeof SettingsChoiceButtonComponent;
