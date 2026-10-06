import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface AccountQuotaData {
  success: boolean;
  tiers: { name: string; utilization: number; resetsAt?: string | null }[];
}

interface QuotaState {
  accountId: string;
  loading: boolean;
  quota: AccountQuotaData | null;
  failed: boolean;
  updatedAt: number | null;
}

function decodeQuota(value: unknown): AccountQuotaData {
  if (!value || typeof value !== "object") throw new Error("Invalid quota response");
  const result = value as AccountQuotaData;
  if (typeof result.success !== "boolean" || !Array.isArray(result.tiers)) throw new Error("Invalid quota response");
  return {
    success: result.success,
    tiers: result.tiers
      .filter(
        (tier) => tier && typeof tier.name === "string" && Number.isFinite(tier.utilization) && tier.utilization >= 0,
      )
      .map((tier) => ({
        name: tier.name,
        utilization: Math.min(100, tier.utilization),
        resetsAt: typeof tier.resetsAt === "string" ? tier.resetsAt : null,
      })),
  };
}

function emptyState(accountId: string): QuotaState {
  return { accountId, loading: true, quota: null, failed: false, updatedAt: null };
}

export function useAccountQuota(accountId: string, refreshKey = 0, onFailure?: () => void) {
  const [state, setState] = useState<QuotaState>(() => emptyState(accountId));
  const [retry, setRetry] = useState(0);
  const failure = useRef(onFailure);
  failure.current = onFailure;

  useEffect(() => {
    let active = true;
    setState((previous) =>
      previous.accountId === accountId ? { ...previous, loading: true, failed: false } : emptyState(accountId),
    );
    void invoke<unknown>("get_codex_oauth_quota", { accountId })
      .then((value) => {
        if (!active) return;
        const quota = decodeQuota(value);
        if (!quota.success) throw new Error("Quota query failed");
        setState({ accountId, loading: false, quota, failed: false, updatedAt: Date.now() });
      })
      .catch(() => {
        if (!active) return;
        setState((previous) => ({ ...previous, loading: false, failed: true }));
        failure.current?.();
      });
    return () => {
      active = false;
    };
  }, [accountId, refreshKey, retry]);

  return {
    ...(state.accountId === accountId ? state : emptyState(accountId)),
    retry: () => setRetry((value) => value + 1),
  };
}
