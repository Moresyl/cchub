import { Button } from "../ui/button";
import { useAccountQuota } from "./useAccountQuota";

interface Props {
  accountId: string;
  localeText: (zh: string, en: string, ja?: string) => string;
  onFailure?: () => void;
  refreshKey?: number;
}

export default function AccountQuota({ accountId, localeText: tx, onFailure, refreshKey }: Props) {
  const state = useAccountQuota(accountId, refreshKey, onFailure);
  if (state.loading && !state.quota)
    return (
      <p role="status" className="text-xs text-muted-foreground">
        {tx("配额读取中…", "Loading quota…", "割当を読み込み中…")}
      </p>
    );
  const failure = state.failed && (
    <div className="flex flex-wrap items-center gap-2">
      <p role="status" className="text-xs text-[var(--warning)]">
        {state.quota
          ? tx(
              "更新失败，以下为上次成功读取的数据。",
              "Refresh failed. Last successful result below.",
              "更新に失敗しました。以下は前回成功時のデータです。",
            )
          : tx("配额查询失败", "Quota query failed", "割当の照会に失敗しました")}
      </p>
      <Button variant="ghost" size="sm" onClick={state.retry}>
        {tx("重试配额查询", "Retry quota query", "割当を再照会")}
      </Button>
    </div>
  );
  if (state.failed && !state.quota) return failure;
  const tiers = state.quota?.tiers ?? [];
  return (
    <div className="grid min-w-0 gap-2">
      {failure}
      <div role="status" className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted-foreground">
        {state.loading && <span>{tx("正在更新…", "Refreshing…", "更新中…")}</span>}
        {state.updatedAt !== null && (
          <span>
            {tx("更新于", "Updated", "更新時刻")}:{" "}
            <time dateTime={new Date(state.updatedAt).toISOString()}>
              {new Date(state.updatedAt).toLocaleTimeString()}
            </time>
          </span>
        )}
      </div>
      {!tiers.length && (
        <p className="text-xs text-muted-foreground">
          {tx("供应商未返回配额数据", "No quota data reported", "割当データがありません")}
        </p>
      )}
      {tiers.map((tier, index) => {
        const used = Math.max(0, Math.min(100, tier.utilization));
        const name =
          tier.name === "five_hour"
            ? tx("5 小时额度", "5-hour quota", "5時間の割当")
            : ["seven_day", "weekly"].includes(tier.name)
              ? tx("7 天额度", "7-day quota", "7日間の割当")
              : tier.name.replace(/_/g, " ");
        const reset = tier.resetsAt ? new Date(tier.resetsAt) : null;
        return (
          <div key={`${tier.name}:${index}`} className="space-y-1">
            <div className="flex min-w-0 items-start justify-between gap-3 text-xs text-muted-foreground">
              <span className="break-words">{name}</span>
              <span className="shrink-0 tabular-nums">{used.toFixed(0)}%</span>
            </div>
            {reset && Number.isFinite(reset.getTime()) && (
              <p className="break-words text-[11px] text-muted-foreground">
                {tx("重置于", "Resets", "リセット時刻")}:{" "}
                <time dateTime={reset.toISOString()}>{reset.toLocaleString()}</time>
              </p>
            )}
            <div
              role="progressbar"
              aria-label={name}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={used}
              className="h-1 overflow-hidden rounded-sm bg-[var(--border-default)]"
            >
              <div
                className="h-full"
                style={{
                  width: `${used}%`,
                  background: used >= 90 ? "var(--danger)" : used >= 70 ? "var(--warning)" : "var(--text-secondary)",
                }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
