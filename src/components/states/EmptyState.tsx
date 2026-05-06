import type { ReactNode } from "react";
import { Inbox } from "lucide-react";

interface EmptyStateProps {
  title: string;
  description?: string;
  icon?: ReactNode;
  action?: ReactNode;
}

export default function EmptyState({ title, description, icon, action }: EmptyStateProps) {
  return (
    <div className="empty-state state-panel">
      <div className="empty-icon">{icon ?? <Inbox size={26} />}</div>
      <div className="state-title">{title}</div>
      {description ? <div className="state-copy">{description}</div> : null}
      {action}
    </div>
  );
}
