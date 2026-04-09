import { memo } from "react";
import { Check } from "lucide-react";

interface ProfileTargetToolToggleProps {
  toolId: string;
  toolName: string;
  selected: boolean;
  disabled: boolean;
  onToggle: (toolId: string) => void | Promise<void>;
}

function ProfileTargetToolToggleComponent({
  toolId,
  toolName,
  selected,
  disabled,
  onToggle,
}: ProfileTargetToolToggleProps) {
  return (
    <button
      type="button"
      className={`btn btn-sm ${selected ? "btn-primary" : "btn-secondary"}`}
      onClick={() => onToggle(toolId)}
      disabled={disabled}
      style={{ gap: 6 }}
    >
      {toolName}
      {selected && <Check size={12} />}
    </button>
  );
}

export default memo(ProfileTargetToolToggleComponent);
