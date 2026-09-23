import type { ReactNode } from "react";
import { AlertTriangle } from "lucide-react";
import { Button } from "../ui/button";

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
        <Button variant="secondary" size="sm" type="button" onClick={onRetry}>
          {retryLabel ?? "Retry"}
        </Button>
      ) : (
        action
      )}
    </div>
  );
}
