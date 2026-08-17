import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Activity, BarChart3, CalendarDays, Database, RefreshCw, TrendingUp } from "lucide-react";
import { getLocale } from "../lib/i18n";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import ModelsDevSyncPanel from "../components/ModelsDevSyncPanel";

interface UsageSummary {
  total_requests: number;
  success_requests: number;
  success_rate: number;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  total_cost_usd: string;
}

interface UsageTrendPoint {
  date: string;
  requests: number;
  success_requests: number;
  input_tokens: number;
  output_tokens: number;
  total_cost_usd: string;
}

interface UsageProviderRow {
  provider_name: string;
  app_id: string;
  requests: number;
  success_rate: number;
  total_tokens: number;
  total_cost_usd: string;
  avg_latency_ms: number;
}

interface UsageModelRow {
  model: string;
  requests: number;
  success_rate: number;
  total_tokens: number;
  total_cost_usd: string;
  avg_latency_ms: number;
}

interface UsageAnalytics {
  days: number;
  start_date: string;
  end_date: string;
  summary: UsageSummary;
  trends: UsageTrendPoint[];
  providers: UsageProviderRow[];
  models: UsageModelRow[];
}

const APP_OPTIONS = [
  ["", "全部应用", "All apps", "すべてのアプリ"],
  ["claude", "Claude", "Claude", "Claude"],
  ["codex", "Codex", "Codex", "Codex"],
  ["gemini", "Gemini", "Gemini", "Gemini"],
  ["grokbuild", "Grok Build", "Grok Build", "Grok Build"],
  ["opencode", "OpenCode", "OpenCode", "OpenCode"],
  ["openclaw", "OpenClaw", "OpenClaw", "OpenClaw"],
  ["hermes", "Hermes", "Hermes", "Hermes"],
  ["pi", "Pi", "Pi", "Pi"],
] as const;

const RANGE_OPTIONS = [
  [1, "今天", "Today", "今日"],
  [7, "近 7 天", "Last 7 days", "過去 7 日"],
  [30, "近 30 天", "Last 30 days", "過去 30 日"],
  [90, "近 90 天", "Last 90 days", "過去 90 日"],
] as const;

function number(value: number) {
  return Intl.NumberFormat("en-US").format(value);
}

function cost(value: string) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? `$${parsed.toFixed(parsed >= 1 ? 4 : 6)}` : "$0.000000";
}

function percent(value: number) {
  return `${value.toFixed(1)}%`;
}

export default function Usage() {
  const locale = getLocale();
  const uiText = useCallback(
    (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en),
    [locale],
  );
  const [days, setDays] = useState(7);
  const [appId, setAppId] = useState("");
  const [providerName, setProviderName] = useState("");
  const [model, setModel] = useState("");
  const [data, setData] = useState<UsageAnalytics | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = await invoke<UsageAnalytics>("get_usage_analytics", {
        days,
        appId: appId || null,
        providerName: providerName || null,
        model: model || null,
      });
      setData(next);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  }, [appId, days, model, providerName]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let disposed = false;
    const subscription = listen("usage-log-recorded", () => {
      if (!disposed) void load();
    });
    return () => {
      disposed = true;
      void subscription.then((unlisten) => unlisten());
    };
  }, [load]);

  const providerOptions = useMemo(() => {
    const names = new Set((data?.providers ?? []).map((item) => item.provider_name));
    if (providerName) names.add(providerName);
    return [...names].sort((left, right) => left.localeCompare(right));
  }, [data?.providers, providerName]);

  const modelOptions = useMemo(() => {
    const names = new Set((data?.models ?? []).map((item) => item.model));
    if (model) names.add(model);
    return [...names].sort((left, right) => left.localeCompare(right));
  }, [data?.models, model]);

  const maxRequests = Math.max(1, ...(data?.trends ?? []).map((item) => item.requests));

  if (loading && !data) {
    return (
      <LoadingState label={uiText("正在加载用量分析...", "Loading usage analytics...", "使用量分析を読み込み中...")} />
    );
  }

  if (error && !data) {
    return (
      <ErrorState
        title={uiText("用量分析加载失败", "Usage analytics failed", "使用量分析の読み込みに失敗しました")}
        message={error}
        retryLabel={uiText("重试", "Retry", "再試行")}
        onRetry={() => void load()}
      />
    );
  }

  const summary = data?.summary;
  return (
    <div className="page-stack">
      <div className="page-header">
        <div>
          <div className="page-title-row">
            <BarChart3 size={19} />
            <h1 className="page-title">{uiText("用量分析", "Usage Analytics", "使用量分析")}</h1>
          </div>
          <p className="page-subtitle">
            {uiText(
              "按时间、应用、Provider 和模型聚合本地代理用量，数据不会离开本机。",
              "Aggregate local proxy usage by time, app, provider, and model. Data stays on this device.",
              "ローカルプロキシの使用量を期間、アプリ、Provider、モデル別に集計します。データは端末内に保存されます。",
            )}
          </p>
        </div>
        <button className="btn btn-secondary btn-sm" type="button" onClick={() => void load()} disabled={loading}>
          <RefreshCw size={14} className={loading ? "spin" : undefined} />
          {uiText("刷新", "Refresh", "更新")}
        </button>
      </div>

      <div className="section-card" style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "center" }}>
        <CalendarDays size={15} style={{ color: "var(--text-secondary)" }} />
        <div className="segmented-control" role="group" aria-label={uiText("时间范围", "Date range", "期間")}>
          {RANGE_OPTIONS.map(([value, zh, en, ja]) => (
            <button key={value} type="button" className={days === value ? "active" : ""} onClick={() => setDays(value)}>
              {uiText(zh, en, ja)}
            </button>
          ))}
        </div>
        <select
          className="input"
          value={appId}
          onChange={(event) => {
            setAppId(event.target.value);
            setProviderName("");
            setModel("");
          }}
        >
          {APP_OPTIONS.map(([value, zh, en, ja]) => (
            <option key={value} value={value}>
              {uiText(zh, en, ja)}
            </option>
          ))}
        </select>
        <select
          className="input"
          value={providerName}
          onChange={(event) => {
            setProviderName(event.target.value);
            setModel("");
          }}
        >
          <option value="">{uiText("全部 Provider", "All providers", "すべての Provider")}</option>
          {providerOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
        <select className="input" value={model} onChange={(event) => setModel(event.target.value)}>
          <option value="">{uiText("全部模型", "All models", "すべてのモデル")}</option>
          {modelOptions.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
      </div>

      {error ? <div className="inline-error">{error}</div> : null}
      <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
        {data ? `${data.start_date} → ${data.end_date}` : "--"}
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))", gap: 10 }}>
        <Metric
          icon={<Activity size={15} />}
          label={uiText("请求数", "Requests", "リクエスト")}
          value={number(summary?.total_requests ?? 0)}
        />
        <Metric
          icon={<TrendingUp size={15} />}
          label={uiText("成功率", "Success rate", "成功率")}
          value={percent(summary?.success_rate ?? 0)}
        />
        <Metric
          icon={<Database size={15} />}
          label={uiText("总 Tokens", "Total tokens", "合計 Tokens")}
          value={number(
            (summary?.input_tokens ?? 0) +
              (summary?.output_tokens ?? 0) +
              (summary?.cache_read_tokens ?? 0) +
              (summary?.cache_creation_tokens ?? 0),
          )}
        />
        <Metric
          icon={<BarChart3 size={15} />}
          label={uiText("累计成本", "Total cost", "合計コスト")}
          value={cost(summary?.total_cost_usd ?? "0")}
        />
      </div>

      <div className="section-card">
        <div className="section-card-title">
          <TrendingUp size={16} />
          {uiText("每日趋势", "Daily trend", "日別トレンド")}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 8, marginTop: 14 }}>
          {(data?.trends ?? []).map((point) => (
            <div
              key={point.date}
              style={{
                display: "grid",
                gridTemplateColumns: "86px minmax(80px, 1fr) 76px 76px",
                gap: 10,
                alignItems: "center",
                fontSize: 12,
              }}
            >
              <span style={{ color: "var(--text-muted)" }}>{point.date.slice(5)}</span>
              <div style={{ height: 8, background: "var(--bg-input)", borderRadius: 4, overflow: "hidden" }}>
                <div
                  style={{
                    width: `${Math.max(2, (point.requests / maxRequests) * 100)}%`,
                    height: "100%",
                    background: "var(--accent)",
                    borderRadius: 4,
                  }}
                />
              </div>
              <span style={{ textAlign: "right" }}>
                {number(point.requests)} {uiText("次", "req", "回")}
              </span>
              <span style={{ textAlign: "right", color: "var(--text-muted)" }}>{cost(point.total_cost_usd)}</span>
            </div>
          ))}
          {(data?.trends.length ?? 0) === 0 ? (
            <div className="empty-state">
              <div className="state-copy">{uiText("暂无用量记录", "No usage records", "使用量の記録はありません")}</div>
            </div>
          ) : null}
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: 12 }}>
        <AggregateTable
          title={uiText("Provider 排名", "Provider ranking", "Provider ランキング")}
          headers={[
            uiText("Provider", "Provider", "Provider"),
            uiText("请求", "Requests", "リクエスト"),
            uiText("成功率", "Success", "成功率"),
            uiText("成本", "Cost", "コスト"),
          ]}
          rows={(data?.providers ?? []).map((item) => [
            item.provider_name,
            `${item.requests}`,
            percent(item.success_rate),
            cost(item.total_cost_usd),
          ])}
          empty={uiText("暂无 Provider 数据", "No provider data", "Provider データなし")}
        />
        <AggregateTable
          title={uiText("模型排名", "Model ranking", "モデルランキング")}
          headers={[
            uiText("模型", "Model", "モデル"),
            uiText("请求", "Requests", "リクエスト"),
            uiText("平均延迟", "Avg latency", "平均遅延"),
            uiText("成本", "Cost", "コスト"),
          ]}
          rows={(data?.models ?? []).map((item) => [
            item.model,
            `${item.requests}`,
            `${item.avg_latency_ms}ms`,
            cost(item.total_cost_usd),
          ])}
          empty={uiText("暂无模型数据", "No model data", "モデルデータなし")}
        />
      </div>
      <ModelsDevSyncPanel />
    </div>
  );
}

function Metric({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="section-card" style={{ padding: 14 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--text-muted)", fontSize: 11 }}>
        {icon}
        {label}
      </div>
      <div style={{ fontSize: 21, fontWeight: 700, marginTop: 7 }}>{value}</div>
    </div>
  );
}

function AggregateTable({
  title,
  headers,
  rows,
  empty,
}: {
  title: string;
  headers: string[];
  rows: string[][];
  empty: string;
}) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <BarChart3 size={16} />
        {title}
      </div>
      {rows.length === 0 ? (
        <div className="state-copy" style={{ marginTop: 14 }}>
          {empty}
        </div>
      ) : (
        <div style={{ overflowX: "auto", marginTop: 10 }}>
          <table className="data-table">
            <thead>
              <tr>
                {headers.map((header) => (
                  <th key={header}>{header}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.slice(0, 12).map((row, index) => (
                <tr key={`${row[0]}-${index}`}>
                  {row.map((cell, cellIndex) => (
                    <td
                      key={`${row[0]}-${cellIndex}`}
                      style={
                        cellIndex === 0
                          ? { maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }
                          : undefined
                      }
                    >
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
