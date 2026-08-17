import { memo, useCallback, useEffect, useState } from "react";
import { Activity, RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

type LocaleText = (zh: string, en: string, ja?: string) => string;

interface HealthItem {
  profileId: string;
  providerName: string;
  toolId: string;
  endpoint?: string | null;
  status: string;
  latencyMs?: number | null;
  httpStatus?: number | null;
  message: string;
}

interface StatsItem {
  providerName: string;
  requests: number;
  successfulRequests: number;
  successRate: number;
  averageLatencyMs: number;
}

interface Props {
  localeText: LocaleText;
}

function statusClass(status: string) {
  if (status === "healthy") return "badge-success";
  if (status === "reachable" || status === "degraded") return "badge-accent";
  return "badge-muted";
}

export default memo(function ProviderHealthPanel({ localeText }: Props) {
  const [health, setHealth] = useState<HealthItem[]>([]);
  const [stats, setStats] = useState<StatsItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [nextHealth, nextStats] = await Promise.all([
        invoke<HealthItem[]>("get_provider_health", { providerId: null }),
        invoke<StatsItem[]>("get_provider_stats"),
      ]);
      setHealth(nextHealth);
      setStats(nextStats);
      setMessage("");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <div className="card" style={{ padding: 14, display: "grid", gap: 10 }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "center" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 7, fontWeight: 700, fontSize: 14 }}>
          <Activity size={16} />
          {localeText("Provider 健康与统计", "Provider health & stats", "Provider ヘルスと統計")}
        </div>
        <button className="btn btn-secondary btn-sm" onClick={() => void refresh()} disabled={loading}>
          <RefreshCw size={13} className={loading ? "spin" : undefined} />
          {localeText("刷新", "Refresh", "更新")}
        </button>
      </div>
      {message && <div style={{ color: "var(--text-muted)", fontSize: 11 }}>{message}</div>}
      {health.length === 0 ? (
        <div style={{ color: "var(--text-muted)", fontSize: 11 }}>
          {localeText("暂无配置可探测", "No configured providers to probe", "確認可能な Provider はありません")}
        </div>
      ) : (
        <div style={{ display: "grid", gap: 6 }}>
          {health.slice(0, 8).map((item) => (
            <div
              key={item.profileId}
              style={{
                display: "flex",
                justifyContent: "space-between",
                gap: 8,
                alignItems: "center",
                padding: "7px 9px",
                background: "var(--bg-input)",
                borderRadius: 7,
              }}
            >
              <div style={{ minWidth: 0 }}>
                <div style={{ fontSize: 12, fontWeight: 600 }}>{item.providerName}</div>
                <div style={{ color: "var(--text-muted)", fontSize: 10 }}>
                  {item.toolId}
                  {item.latencyMs != null ? ` · ${item.latencyMs}ms` : ""}
                </div>
              </div>
              <span className={`badge ${statusClass(item.status)}`}>{item.status}</span>
            </div>
          ))}
        </div>
      )}
      {stats.length > 0 && (
        <div style={{ color: "var(--text-muted)", fontSize: 10 }}>
          {stats
            .slice(0, 3)
            .map((item) => `${item.providerName}: ${item.successRate.toFixed(1)}% / ${item.requests}`)
            .join(" · ")}
        </div>
      )}
    </div>
  );
});
