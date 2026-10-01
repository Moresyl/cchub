import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "../ui/button";

interface Quota {
  success: boolean;
  tiers: { name: string; utilization: number; resetsAt?: string | null }[];
}

interface Props {
  accountId: string;
  localeText: (zh: string, en: string, ja?: string) => string;
  onFailure?: () => void;
}

export default function AccountQuota({ accountId, localeText: tx, onFailure }: Props) {
  const [state, setState] = useState<{ accountId: string; loading: boolean; quota: Quota | null; failed: boolean }>(
    () => ({ accountId, loading: true, quota: null, failed: false }),
  );
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let active = true;
    setState({ accountId, loading: true, quota: null, failed: false });
    void invoke<Quota>("get_codex_oauth_quota", { accountId })
      .then((quota) => {
        if (!active) return;
        if (!quota || !Array.isArray(quota.tiers)) throw new Error("Invalid quota response");
        setState({ accountId, loading: false, quota, failed: !quota.success });
        if (!quota.success) onFailure?.();
      })
      .catch(() => {
        if (!active) return;
        setState({ accountId, loading: false, quota: null, failed: true });
        onFailure?.();
      });
    return () => {
      active = false;
    };
  }, [accountId, retry, onFailure]);
  if (state.accountId !== accountId || state.loading)
    return (
      <p role="status" className="text-xs text-muted-foreground">
        {tx("配额读取中…", "Loading quota…", "割当を読み込み中…")}
      </p>
    );
  if (state.failed)
    return (
      <div className="flex flex-wrap items-center gap-2">
        <p role="status" className="text-xs text-[var(--warning)]">
          {tx("配额查询失败", "Quota query failed", "割当の照会に失敗しました")}
        </p>
        <Button variant="ghost" onClick={() => setRetry((value) => value + 1)}>
          {tx("重试配额查询", "Retry quota query", "割当を再照会")}
        </Button>
      </div>
    );
  const tiers =
    state.quota?.tiers.filter(
      (tier) => tier && typeof tier.name === "string" && Number.isFinite(tier.utilization) && tier.utilization >= 0,
    ) ?? [];
  if (!tiers.length)
    return (
      <p className="text-xs text-muted-foreground">
        {tx("供应商未返回配额数据", "No quota data reported", "割当データがありません")}
      </p>
    );
  return (
    <div className="grid min-w-0 gap-2">
      {tiers.map((tier, index) => {
        const used = Math.max(0, Math.min(100, tier.utilization));
        const name =
          tier.name === "five_hour"
            ? tx("5 小时额度", "5-hour quota", "5時間の割当")
            : ["seven_day", "weekly"].includes(tier.name)
              ? tx("7 天额度", "7-day quota", "7日間の割当")
              : tier.name.replace(/_/g, " ");
        return (
          <div
            key={`${tier.name}:${index}`}
            className="space-y-1"
            title={tier.resetsAt ? `${name}: ${tier.resetsAt}` : name}
          >
            <div className="flex min-w-0 items-start justify-between gap-3 text-xs text-muted-foreground">
              <span className="break-words">{name}</span>
              <span className="shrink-0 tabular-nums">{used.toFixed(0)}%</span>
            </div>
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
