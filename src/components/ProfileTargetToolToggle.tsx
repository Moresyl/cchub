import { memo } from "react";
import { Check } from "lucide-react";
import { Button } from "./ui/button";

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
    <Button
      type="button"
      variant={selected ? "default" : "secondary"}
      aria-pressed={selected}
      onClick={() => onToggle(toolId)}
      disabled={disabled}
      style={{ gap: 6 }}
    >
      {toolName}
      {selected && <Check size={12} />}
    </Button>
  );
}

export default memo(ProfileTargetToolToggleComponent);
