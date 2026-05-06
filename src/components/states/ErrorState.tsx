import type { ReactNode } from "react";
import { AlertTriangle } from "lucide-react";

interface ErrorStateProps {
  title: string;
  message: string;
  retryLabel?: string;
  onRetry?: () => void;
  action?: ReactNode;
}

export default function ErrorState({ title, message, retryLabel, onRetry, action }: ErrorStateProps) {
  return (
    <div className="empty-state state-panel" role="alert">
      <div className="empty-icon state-icon-danger">
        <AlertTriangle size={26} />
      </div>
      <div className="state-title">{title}</div>
      <div className="state-copy">{message}</div>
      {onRetry ? (
        <button className="btn btn-secondary btn-sm" type="button" onClick={onRetry}>
          {retryLabel ?? "Retry"}
        </button>
      ) : (
        action
      )}
    </div>
  );
}
