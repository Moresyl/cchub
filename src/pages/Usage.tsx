import { useCallback, useMemo, useState } from "react";
import { Activity, BarChart3, CalendarDays, Database, RefreshCw, TrendingUp } from "lucide-react";
import { getLocale } from "../lib/i18n";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import ModelsDevSyncPanel from "../components/ModelsDevSyncPanel";
import { Button } from "../components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/ui/select";

import { useUsageAnalytics } from "./usage/useUsageAnalytics";
import { AggregateTable } from "./usage/AggregateTable";
import "./usage/usage.css";

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

const ALL_FILTER = "__all__";

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
  const {
    data,
    loading,
    error,
    refresh: load,
    liveUnavailable,
    providers,
    models,
  } = useUsageAnalytics({
    days,
    appId,
    providerName,
    model,
  });
  const filterKey = JSON.stringify([days, appId, providerName, model]);

  const providerOptions = useMemo(() => {
    const names = new Set(providers);
    if (providerName) names.add(providerName);
    return [...names].sort((left, right) => left.localeCompare(right));
  }, [providers, providerName]);

  const modelOptions = useMemo(() => {
    const names = new Set(models);
    if (model) names.add(model);
    return [...names].sort((left, right) => left.localeCompare(right));
  }, [models, model]);

  const maxRequests = Math.max(1, ...(data?.trends ?? []).map((item) => item.requests));

  const summary = data?.summary;
  return (
    <div className="usage-page flex min-w-0 flex-col gap-4">
      <div className="page-header">
        <div>
          <div className="flex items-center gap-2">
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
        <Button variant="secondary" size="sm" type="button" onClick={() => void load()} disabled={loading}>
          <RefreshCw size={14} className={loading ? "spin" : undefined} />
          {uiText("刷新", "Refresh", "更新")}
        </Button>
      </div>

      <div className="space-y-3 border-b border-border pb-4">
        <div className="flex flex-wrap items-center gap-2">
          <CalendarDays size={15} className="shrink-0 text-muted-foreground" aria-hidden="true" />
          <div
            className="inline-flex flex-wrap items-center gap-1 rounded-md border border-border bg-[var(--bg-input)] p-1"
            role="group"
            aria-label={uiText("时间范围", "Date range", "期間")}
          >
            {RANGE_OPTIONS.map(([value, zh, en, ja]) => (
              <Button
                key={value}
                type="button"
                variant={days === value ? "secondary" : "ghost"}
                size="sm"
                className="h-7 px-2.5"
                aria-pressed={days === value}
                onClick={() => setDays(value)}
              >
                {uiText(zh, en, ja)}
              </Button>
            ))}
          </div>
        </div>
        <div className="grid min-w-0 gap-2 sm:grid-cols-3">
          <Select
            value={appId || ALL_FILTER}
            onValueChange={(value) => {
              setAppId(value === ALL_FILTER ? "" : value);
              setProviderName("");
              setModel("");
            }}
          >
            <SelectTrigger aria-label={uiText("应用", "App", "アプリ")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {APP_OPTIONS.map(([value, zh, en, ja]) => (
                <SelectItem key={value} value={value || ALL_FILTER}>
                  {uiText(zh, en, ja)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={providerName || ALL_FILTER}
            onValueChange={(value) => {
              setProviderName(value === ALL_FILTER ? "" : value);
              setModel("");
            }}
          >
            <SelectTrigger aria-label={uiText("Provider", "Provider", "Provider")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL_FILTER}>
                {uiText("全部 Provider", "All providers", "すべての Provider")}
              </SelectItem>
              {providerOptions.map((value) => (
                <SelectItem key={value} value={value}>
                  {value}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={model || ALL_FILTER} onValueChange={(value) => setModel(value === ALL_FILTER ? "" : value)}>
            <SelectTrigger aria-label={uiText("模型", "Model", "モデル")}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={ALL_FILTER}>{uiText("全部模型", "All models", "すべてのモデル")}</SelectItem>
              {modelOptions.map((value) => (
                <SelectItem key={value} value={value}>
                  {value}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {liveUnavailable ? (
        <p className="state-copy" role="status">
          {uiText(
            "实时刷新暂不可用，可以手动刷新用量。",
            "Live updates are unavailable. You can still refresh manually.",
            "自動更新を利用できません。手動で更新できます。",
          )}
        </p>
      ) : null}
      {loading && !data ? (
        <LoadingState
          label={uiText("正在加载用量分析...", "Loading usage analytics...", "使用量分析を読み込み中...")}
        />
      ) : null}
      {error && !data ? (
        <ErrorState
          title={uiText("用量分析加载失败", "Usage analytics failed", "使用量分析の読み込みに失敗しました")}
          message={uiText(
            "无法读取本地用量，请重试。",
            "Could not read local usage. Please retry.",
            "ローカルの使用量を読み込めませんでした。再試行してください。",
          )}
          retryLabel={uiText("重试", "Retry", "再試行")}
          onRetry={load}
        />
      ) : null}
      {data ? (
        <div className="usage-results" aria-busy={loading}>
          {error ? (
            <div className="inline-error" role="alert">
              {uiText(
                "刷新失败，以下仍是上次成功读取的数据。请重试。",
                "Refresh failed. Showing the last successful result. Please retry.",
                "更新に失敗しました。前回のデータを表示しています。再試行してください。",
              )}
            </div>
          ) : null}
          <div className="usage-period" role="status">
            {`${data.start_date} → ${data.end_date}`}
            {loading ? <span>{uiText("正在刷新…", "Refreshing…", "更新中…")}</span> : null}
          </div>

          <div className="usage-metrics">
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
                summary?.total_tokens ??
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

          <section className="section-card">
            <h2 className="section-card-title">
              <TrendingUp size={16} aria-hidden="true" />
              {uiText("每日趋势", "Daily trend", "日別トレンド")}
            </h2>
            <div
              className="usage-trend-scroll"
              role="region"
              aria-label={uiText("每日趋势滚动区", "Daily trend scroll area", "日別トレンドのスクロール領域")}
              tabIndex={0}
            >
              <ul className="usage-trends" aria-label={uiText("每日趋势", "Daily trend", "日別トレンド")}>
                {(summary?.total_requests ?? 0) > 0 ? (
                  data.trends.map((point) => (
                    <li key={point.date} className="usage-trend-row">
                      <time dateTime={point.date} aria-label={point.date}>
                        {point.date.slice(5)}
                      </time>
                      <div className="usage-trend-track" aria-hidden="true">
                        <div
                          style={{
                            width: `${point.requests > 0 ? Math.max(2, (point.requests / maxRequests) * 100) : 0}%`,
                            height: "100%",
                            background: "var(--accent)",
                            borderRadius: 4,
                          }}
                        />
                      </div>
                      <div className="usage-trend-detail">
                        <span>
                          {number(point.requests)} {uiText("次", "req", "回")}
                        </span>
                        <span className="text-muted-foreground">{cost(point.total_cost_usd)}</span>
                      </div>
                    </li>
                  ))
                ) : (
                  <li className="state-copy py-8 text-center">
                    {uiText("暂无用量记录", "No usage records", "使用量の記録はありません")}
                  </li>
                )}
              </ul>
            </div>
          </section>

          <div className="usage-rankings">
            <AggregateTable
              key={`${filterKey}-providers`}
              uiText={uiText}
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
              key={`${filterKey}-models`}
              uiText={uiText}
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
        </div>
      ) : null}
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
      <div className="usage-metric-value">{value}</div>
    </div>
  );
}
