import { memo } from "react";

interface ProfilePresetButtonProps {
  presetId: string;
  name: string;
  badge?: string | null;
  active: boolean;
  onApply: (presetId: string) => void | Promise<void>;
}

function ProfilePresetButtonComponent({
  presetId,
  name,
  badge,
  active,
  onApply,
}: ProfilePresetButtonProps) {
  return (
    <button
      className={`btn btn-sm ${active ? "btn-primary" : "btn-secondary"}`}
      onClick={() => onApply(presetId)}
      style={{ gap: 4 }}
    >
      {name}
      {badge && (
        <span style={{ fontSize: 10, opacity: 0.7, fontWeight: 400 }}>({badge})</span>
      )}
    </button>
  );
}

export default memo(ProfilePresetButtonComponent);
