import { AlertTriangle } from "lucide-react";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";

interface RefreshErrorProps {
  title: string;
  message: string;
  retryLabel: string;
  onRetry: () => void;
}

export default function RefreshError({ title, message, retryLabel, onRetry }: RefreshErrorProps) {
  return (
    <Card role="alert" className="flex shrink-0 flex-wrap items-center gap-3 p-3">
      <AlertTriangle size={16} className="shrink-0 text-[var(--warning)]" aria-hidden="true" />
      <div className="min-w-0 flex-1 text-xs">
        <p className="font-[590]">{title}</p>
        <p className="break-words text-muted-foreground">{message}</p>
      </div>
      <Button variant="secondary" onClick={onRetry}>
        {retryLabel}
      </Button>
    </Card>
  );
}
