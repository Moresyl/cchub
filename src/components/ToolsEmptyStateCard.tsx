import { memo } from "react";

interface ToolsEmptyStateCardProps {
  title: string;
  description: string;
  marginBottom?: number;
}

function ToolsEmptyStateCardComponent({
  title,
  description,
  marginBottom,
}: ToolsEmptyStateCardProps) {
  return (
    <div className="card" style={{ padding: "40px 20px", textAlign: "center", marginBottom }}>
      <p style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
        {title}
      </p>
      <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
        {description}
      </p>
    </div>
  );
}

export default memo(ToolsEmptyStateCardComponent);
