import { memo } from "react";
import { Button } from "./ui/button";

interface ProfileToolFilterTabProps {
  toolId: string;
  toolName: string;
  count: number;
  active: boolean;
  dimmed: boolean;
  onToggle: (toolId: string) => void | Promise<void>;
}

function ProfileToolFilterTabComponent({
  toolId,
  toolName,
  count,
  active,
  dimmed,
  onToggle,
}: ProfileToolFilterTabProps) {
  return (
    <Button
      variant={active ? "secondary" : "ghost"}
      size="sm"
      className={`profile-tool-tab ${active ? "active" : ""}`}
      role="tab"
      aria-selected={active}
      onClick={() => onToggle(toolId)}
      style={{ opacity: dimmed ? 0.55 : 1 }}
    >
      {toolName} ({count})
    </Button>
  );
}

export default memo(ProfileToolFilterTabComponent);
