import { memo } from "react";

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
    <button
      className={`tab-item ${active ? "active" : ""}`}
      onClick={() => onToggle(toolId)}
      style={{ opacity: dimmed ? 0.55 : 1 }}
    >
      {toolName} ({count})
    </button>
  );
}

export default memo(ProfileToolFilterTabComponent);
