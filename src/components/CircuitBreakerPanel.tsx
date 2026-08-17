import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, RefreshCw, RotateCcw } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";

interface CircuitEntry {
  scope: "profile" | "endpoint";
  key: string;
  state: "closed" | "open" | "half_open";
  consecutiveFailures: number;
  consecutiveSuccesses: number;
  retryAfterMs: number | null;
}

interface CircuitStats {
  entries: CircuitEntry[];
  openCount: number;
  halfOpenCount: number;
}

const EMPTY_STATS: CircuitStats = { entries: [], openCount: 0, halfOpenCount: 0 };

export default function CircuitBreakerPanel() {
  const locale = getLocale();
  const [stats, setStats] = useState<CircuitStats>(EMPTY_STATS);
  const [loading, setLoading] = useState(true);
  const [resetting, setResetting] = useState(false);
  const text = useCallback((zh: string, en: string) => (locale === "zh" ? zh : en), [locale]);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setStats(await invoke<CircuitStats>("get_circuit_breaker_stats"));
    } catch (error) {
      showToast("error", `${text("读取熔断状态失败", "Failed to read circuit status")}: ${error}`);
    } finally {
      setLoading(false);
    }
  }, [text]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const reset = async () => {
    setResetting(true);
    try {
      const count = await invoke<number>("reset_circuit_breakers");
      setStats(EMPTY_STATS);
      showToast("success", text(`已重置 ${count} 个熔断状态`, `Reset ${count} circuit states`));
    } catch (error) {
      showToast("error", `${text("重置熔断状态失败", "Failed to reset circuits")}: ${error}`);
    } finally {
      setResetting(false);
    }
  };

  return (
    <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
      <div
        style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 10 }}
      >
        <div>
          <div
            style={{
              fontSize: 12,
              fontWeight: 700,
              color: "var(--text-muted)",
              textTransform: "uppercase",
              letterSpacing: 1,
            }}
          >
            {text("实时熔断状态", "Live Circuit Status")}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 4 }}>
            {text(
              "观察 Profile 与上游端点的健康状态，故障恢复后可安全重置。",
              "Inspect profile and upstream endpoint health; reset after recovery.",
            )}
          </div>
        </div>
        <div style={{ display: "flex", gap: 6 }}>
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => void refresh()}
            disabled={loading || resetting}
            title={text("刷新", "Refresh")}
          >
            <RefreshCw size={13} className={loading ? "animate-spin" : undefined} />
          </button>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => void reset()}
            disabled={resetting || loading || stats.entries.length === 0}
          >
            <RotateCcw size={13} />
            {text("重置", "Reset")}
          </button>
        </div>
      </div>
      <div style={{ display: "flex", gap: 8, marginBottom: 10, flexWrap: "wrap" }}>
        <StatusPill label={text("打开", "Open")} value={stats.openCount} tone="danger" />
        <StatusPill label={text("半开", "Half-open")} value={stats.halfOpenCount} tone="warning" />
        <StatusPill label={text("已追踪", "Tracked")} value={stats.entries.length} tone="neutral" />
      </div>
      {stats.entries.length === 0 ? (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            color: "var(--text-muted)",
            fontSize: 12,
            padding: "10px 0",
          }}
        >
          <Activity size={14} /> {text("当前没有异常熔断记录。", "No circuit anomalies are currently tracked.")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {stats.entries.map((entry) => (
            <div
              key={`${entry.scope}:${entry.key}`}
              style={{
                display: "grid",
                gridTemplateColumns: "72px minmax(0, 1fr) 80px",
                alignItems: "center",
                gap: 8,
                fontSize: 11,
              }}
            >
              <span style={{ color: "var(--text-muted)", textTransform: "uppercase" }}>{entry.scope}</span>
              <span
                title={entry.key}
                style={{
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                  fontFamily: "var(--font-mono)",
                }}
              >
                {entry.key}
              </span>
              <span
                style={{
                  color:
                    entry.state === "open"
                      ? "var(--danger)"
                      : entry.state === "half_open"
                        ? "var(--warning)"
                        : "var(--success)",
                  textAlign: "right",
                }}
              >
                {entry.state === "half_open" ? text("半开", "half-open") : entry.state}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function StatusPill({ label, value, tone }: { label: string; value: number; tone: "danger" | "warning" | "neutral" }) {
  const color = tone === "danger" ? "var(--danger)" : tone === "warning" ? "var(--warning)" : "var(--text-secondary)";
  return (
    <span
      style={{
        border: `1px solid color-mix(in srgb, ${color} 35%, transparent)`,
        borderRadius: 999,
        padding: "3px 8px",
        color,
        fontSize: 11,
      }}
    >
      {label}: {value}
    </span>
  );
}
