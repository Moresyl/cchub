import { memo } from "react";
import { Button } from "./ui/button";

interface ProfilePresetButtonProps {
  presetId: string;
  name: string;
  badge?: string | null;
  active: boolean;
  onApply: (presetId: string) => void | Promise<void>;
}

function ProfilePresetButtonComponent({ presetId, name, badge, active, onApply }: ProfilePresetButtonProps) {
  return (
    <Button
      type="button"
      variant={active ? "default" : "secondary"}
      aria-pressed={active}
      onClick={() => onApply(presetId)}
      style={{ gap: 4 }}
    >
      {name}
      {badge && <span style={{ fontSize: 11, opacity: 0.7, fontWeight: 400 }}>({badge})</span>}
    </Button>
  );
}

export default memo(ProfilePresetButtonComponent);
