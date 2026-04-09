import { memo, type ReactNode } from "react";

interface ToolsTabButtonProps {
  tabId: string;
  label: string;
  icon: ReactNode;
  active: boolean;
  installed: boolean;
  unavailableLabel: string;
  onSelect: (tabId: string) => void;
}

function ToolsTabButtonComponent({
  tabId,
  label,
  icon,
  active,
  installed,
  unavailableLabel,
  onSelect,
}: ToolsTabButtonProps) {
  return (
    <button
      className={`btn btn-sm ${active ? "btn-primary" : "btn-secondary"}`}
      onClick={() => onSelect(tabId)}
      style={{ gap: 6, opacity: installed ? 1 : 0.5 }}
    >
      {icon}
      {label}
      {!installed && (
        <span style={{ fontSize: 9, color: "var(--text-muted)" }}>
          ({unavailableLabel})
        </span>
      )}
    </button>
  );
}

export default memo(ToolsTabButtonComponent);
