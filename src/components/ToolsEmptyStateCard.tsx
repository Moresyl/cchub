import { memo } from "react";
import EmptyState from "./states/EmptyState";

interface ToolsEmptyStateCardProps {
  title: string;
  description: string;
  marginBottom?: number;
}

function ToolsEmptyStateCardComponent({ title, description, marginBottom }: ToolsEmptyStateCardProps) {
  return (
    <div className="card" style={{ padding: "40px 20px", textAlign: "center", marginBottom }}>
      <EmptyState title={title} description={description} />
    </div>
  );
}

export default memo(ToolsEmptyStateCardComponent);
