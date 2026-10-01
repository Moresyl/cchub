import { AlertTriangle } from "lucide-react";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import type { SessionSummary } from "./helpers";

export interface DeleteFailure {
  session: SessionSummary;
  error: string;
}

export default function DeleteFailures({
  failures,
  busy,
  onRetry,
  onDismiss,
  uiText,
}: {
  failures: DeleteFailure[];
  busy: boolean;
  onRetry: () => void;
  onDismiss: () => void;
  uiText: (zh: string, en: string, ja?: string) => string;
}) {
  if (!failures.length) return null;
  return (
    <Card role="alert" className="section-card" style={{ padding: 16 }}>
      <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
        <div className="flex min-w-0 items-start gap-3">
          <AlertTriangle size={16} className="shrink-0 text-[var(--warning)]" />
          <p className="min-w-0 text-[12px]">
            {uiText(
              `${failures.length} 个会话未删除，成功项已移除`,
              `${failures.length} sessions could not be deleted; successful items were removed`,
              `${failures.length} 件を削除できませんでした。成功した項目は除去済みです`,
            )}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="secondary" disabled={busy} onClick={onRetry}>
            {uiText("重试失败项", "Retry failed items", "失敗した項目を再試行")}
          </Button>
          <Button variant="ghost" disabled={busy} onClick={onDismiss}>
            {uiText("关闭提示", "Dismiss", "閉じる")}
          </Button>
        </div>
      </div>
      <ul className="mt-3 max-h-40 space-y-2 overflow-y-auto text-[12px]">
        {failures.map(({ session, error }) => (
          <li key={`${session.tool_id}:${session.source_path}`} className="break-words">
            <span className="font-[510]">{session.title}</span>
            <span className="ml-2 text-muted-foreground">{error}</span>
          </li>
        ))}
      </ul>
    </Card>
  );
}
