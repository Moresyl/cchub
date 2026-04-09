import { memo } from "react";
import { Monitor } from "lucide-react";

interface DashboardToolChipProps {
  toolId: string;
  toolName: string;
  installed: boolean;
  icon: typeof Monitor;
}

function DashboardToolChipComponent({
  toolId,
  toolName,
  installed,
  icon: Icon,
}: DashboardToolChipProps) {
  return (
    <div
      key={toolId}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "10px 16px",
        borderRadius: 6,
        border: "1px solid var(--border-default)",
        background: installed ? "var(--bg-card)" : "transparent",
        opacity: installed ? 1 : 0.4,
      }}
    >
      <Icon size={16} style={{ color: installed ? "var(--text-primary)" : "var(--text-muted)" }} />
      <span style={{ fontSize: 13, fontWeight: 500, color: installed ? "var(--text-primary)" : "var(--text-muted)" }}>
        {toolName}
      </span>
      {installed && <span className="dot dot-active" style={{ width: 6, height: 6 }} />}
    </div>
  );
}

export default memo(DashboardToolChipComponent);
