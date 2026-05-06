/* eslint-disable react-hooks/exhaustive-deps */
import { useQueryClient } from "@tanstack/react-query";
import { memo, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState, startTransition } from "react";
import { Activity, DollarSign, RefreshCw, Save, X } from "lucide-react";
import ActivityLogRow from "../components/ActivityLogRow";
import ModelPricingListItem, { type ModelPricingListItemRow } from "../components/ModelPricingListItem";
import ProxyRequestRow from "../components/ProxyRequestRow";
import { showToast } from "../components/Toast";
import ErrorState from "../components/states/ErrorState";
import LoadingState from "../components/states/LoadingState";
import { getLocale } from "../lib/i18n";
import { fetchLogsPageData, queryKeys } from "../hooks/queries";
import { useDeleteModelPricingMutation, useSaveModelPricingMutation } from "../hooks/mutations";

interface ActivityItem {
  id: number;
  server_id: string;
  server_name: string;
  request_type: string;
  status: string;
  latency_ms: number | null;
  recorded_at: string;
}

interface ProxyUsageSummary {
  total_requests: number;
  success_requests: number;
  success_rate: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cache_read_tokens: number;
  total_cache_creation_tokens: number;
  total_cost_usd: string;
}

interface ProxyRequestLogRow {
  request_id: string;
  tool_id: string;
  profile_id: string;
  provider_name: string;
  request_model: string | null;
  response_model: string | null;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  total_cost_usd: string;
  latency_ms: number;
  status_code: number;
  is_streaming: boolean;
  error_message: string | null;
  created_at: string;
}

interface ModelPricingRow {
  model_id: string;
  normalized_model_id: string;
  input_cost_per_million: string;
  output_cost_per_million: string;
  cache_read_cost_per_million: string;
  cache_write_cost_per_million: string;
  created_at: string;
  updated_at: string;
}

interface ModelPricingDraft {
  model_id: string;
  input_cost_per_million: string;
  output_cost_per_million: string;
  cache_read_cost_per_million: string;
  cache_write_cost_per_million: string;
}

const TOOL_LABELS: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  opencode: "OpenCode",
  openclaw: "OpenClaw",
  hermes: "Hermes",
};

const DEFAULT_PROXY_FILTERS = {
  toolId: "",
  status: "all",
  streamMode: "all",
  providerQuery: "",
  modelQuery: "",
};

const EMPTY_PRICING_DRAFT: ModelPricingDraft = {
  model_id: "",
  input_cost_per_million: "",
  output_cost_per_million: "",
  cache_read_cost_per_million: "",
  cache_write_cost_per_million: "",
};

const AUTO_REFRESH_INTERVALS = [5, 10, 30, 60] as const;
const MAX_MERGED_PROXY_LOG_ROWS = 240;

export default function Logs() {
  const queryClient = useQueryClient();
  const initialDate = todayStr();
  const cachedLogsPageData = queryClient.getQueryData<Awaited<ReturnType<typeof fetchLogsPageData>>>(
    queryKeys.logsPage(initialDate),
  );
  const [activities, setActivities] = useState<ActivityItem[]>(cachedLogsPageData?.activities ?? []);
  const [proxySummary, setProxySummary] = useState<ProxyUsageSummary | null>(cachedLogsPageData?.proxySummary ?? null);
  const [recentProxyLogs, setRecentProxyLogs] = useState<ProxyRequestLogRow[]>(
    cachedLogsPageData?.recentProxyLogs ?? [],
  );
  const [modelPricingRows, setModelPricingRows] = useState<ModelPricingRow[]>(
    cachedLogsPageData?.modelPricingRows ?? [],
  );
  const [loading, setLoading] = useState(!cachedLogsPageData);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [proxyFilters, setProxyFilters] = useState(DEFAULT_PROXY_FILTERS);
  const [pricingDraft, setPricingDraft] = useState<ModelPricingDraft>(EMPTY_PRICING_DRAFT);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [savingPricing, setSavingPricing] = useState(false);
  const [deletingPricingId, setDeletingPricingId] = useState<string | null>(null);
  const [selectedDate] = useState<string>(initialDate);
  const [autoRefreshEnabled, setAutoRefreshEnabled] = useState(false);
  const [autoRefreshInterval, setAutoRefreshInterval] = useState<(typeof AUTO_REFRESH_INTERVALS)[number]>(10);
  const [refreshing, setRefreshing] = useState(false);
  const proxyListRef = useRef<HTMLDivElement | null>(null);
  const loadRequestIdRef = useRef(0);
  const deferredProviderQuery = useDeferredValue(proxyFilters.providerQuery);
  const deferredModelQuery = useDeferredValue(proxyFilters.modelQuery);
  const locale = getLocale();
  const saveModelPricingMutation = useSaveModelPricingMutation();
  const deleteModelPricingMutation = useDeleteModelPricingMutation();
  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );
  const effectiveProxyFilters = useMemo(
    () => ({
      toolId: proxyFilters.toolId,
      status: proxyFilters.status,
      streamMode: proxyFilters.streamMode,
      providerQuery: deferredProviderQuery,
      modelQuery: deferredModelQuery,
    }),
    [deferredModelQuery, deferredProviderQuery, proxyFilters.status, proxyFilters.streamMode, proxyFilters.toolId],
  );
  const proxyLogs = useMemo(
    () => filterProxyLogs(recentProxyLogs, effectiveProxyFilters),
    [effectiveProxyFilters, recentProxyLogs],
  );

  const load = useCallback(
    async (
      options: { silent?: boolean; mergeProxyRows?: boolean; preserveProxyScroll?: boolean; force?: boolean } = {},
    ) => {
      const { silent = false, mergeProxyRows = false, preserveProxyScroll = false, force = false } = options;
      const requestId = loadRequestIdRef.current + 1;
      loadRequestIdRef.current = requestId;
      const previousScrollTop = preserveProxyScroll ? proxyListRef.current?.scrollTop || 0 : 0;
      const previousScrollHeight = preserveProxyScroll ? proxyListRef.current?.scrollHeight || 0 : 0;
      if (!silent) {
        setLoadError(null);
        if (!queryClient.getQueryData(queryKeys.logsPage(selectedDate))) {
          setLoading(true);
        }
      } else {
        setRefreshing(true);
      }
      try {
        const data = await queryClient.fetchQuery({
          queryKey: queryKeys.logsPage(selectedDate),
          queryFn: () => fetchLogsPageData(selectedDate),
          staleTime: force || silent ? 0 : 30_000,
        });
        if (requestId !== loadRequestIdRef.current) {
          return;
        }
        startTransition(() => {
          setActivities(data.activities);
          setProxySummary(data.proxySummary);
          setModelPricingRows(data.modelPricingRows);
          setRecentProxyLogs((current) =>
            mergeProxyRows ? mergeProxyLogs(current, data.recentProxyLogs) : data.recentProxyLogs,
          );
        });
        if (preserveProxyScroll && previousScrollTop > 0) {
          requestAnimationFrame(() => {
            const element = proxyListRef.current;
            if (!element) return;
            const delta = element.scrollHeight - previousScrollHeight;
            element.scrollTop = previousScrollTop + delta;
          });
        }
      } catch (error) {
        if (requestId !== loadRequestIdRef.current) {
          return;
        }
        if (!silent) {
          setLoadError(String(error));
        } else {
          console.error(error);
        }
      } finally {
        if (requestId === loadRequestIdRef.current) {
          if (!silent) {
            setLoading(false);
          } else {
            setRefreshing(false);
          }
        }
      }
    },
    [selectedDate],
  );

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!autoRefreshEnabled) return undefined;
    let timer: number | null = null;

    const start = () => {
      if (timer !== null || document.hidden) {
        return;
      }

      timer = window.setInterval(() => {
        void load({ silent: true, mergeProxyRows: true, preserveProxyScroll: true });
      }, autoRefreshInterval * 1000);
    };

    const stop = () => {
      if (timer !== null) {
        window.clearInterval(timer);
        timer = null;
      }
    };

    const handleVisibilityChange = () => {
      if (document.hidden) {
        stop();
        return;
      }

      start();
    };

    start();
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      stop();
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [autoRefreshEnabled, autoRefreshInterval, load]);

  const hasActiveProxyFilters =
    proxyFilters.toolId !== "" ||
    proxyFilters.status !== "all" ||
    proxyFilters.streamMode !== "all" ||
    proxyFilters.providerQuery.trim() !== "" ||
    proxyFilters.modelQuery.trim() !== "";

  const handleSavePricing = useCallback(async () => {
    if (savingPricing) return;
    setSavingPricing(true);
    try {
      const savedRow = await saveModelPricingMutation.mutateAsync({ entry: pricingDraft });
      setModelPricingRows((current) => [savedRow, ...current.filter((row) => row.model_id !== savedRow.model_id)]);
      showToast("success", uiText("模型定价已保存", "Model pricing saved", "モデル単価を保存しました"));
      setPricingDraft(EMPTY_PRICING_DRAFT);
      setEditingModelId(null);
    } catch (error) {
      console.error(error);
      showToast(
        "error",
        uiText(
          `保存模型定价失败: ${error}`,
          `Failed to save model pricing: ${error}`,
          `モデル単価の保存に失敗しました: ${error}`,
        ),
      );
    } finally {
      setSavingPricing(false);
    }
  }, [pricingDraft, saveModelPricingMutation, savingPricing, uiText]);

  const handleDeletePricing = useCallback(
    async (modelId: string) => {
      setDeletingPricingId(modelId);
      try {
        await deleteModelPricingMutation.mutateAsync({ modelId });
        if (editingModelId === modelId) {
          setPricingDraft(EMPTY_PRICING_DRAFT);
          setEditingModelId(null);
        }
        setModelPricingRows((current) => current.filter((row) => row.model_id !== modelId));
        showToast("success", uiText("模型定价已删除", "Model pricing deleted", "モデル単価を削除しました"));
      } catch (error) {
        console.error(error);
        showToast(
          "error",
          uiText(
            `删除模型定价失败: ${error}`,
            `Failed to delete model pricing: ${error}`,
            `モデル単価の削除に失敗しました: ${error}`,
          ),
        );
      } finally {
        setDeletingPricingId((current) => (current === modelId ? null : current));
      }
    },
    [deleteModelPricingMutation, editingModelId, uiText],
  );

  const startEditingPricing = useCallback((row: ModelPricingRow) => {
    setEditingModelId(row.model_id);
    setPricingDraft({
      model_id: row.model_id,
      input_cost_per_million: row.input_cost_per_million,
      output_cost_per_million: row.output_cost_per_million,
      cache_read_cost_per_million: row.cache_read_cost_per_million,
      cache_write_cost_per_million: row.cache_write_cost_per_million,
    });
  }, []);

  const resetPricingDraft = useCallback(() => {
    setPricingDraft(EMPTY_PRICING_DRAFT);
    setEditingModelId(null);
  }, []);

  const handleRefresh = useCallback(() => {
    void load({ silent: true, mergeProxyRows: true, preserveProxyScroll: true, force: true });
  }, [load]);

  const handleClearProxyFilters = useCallback(() => {
    setProxyFilters({ ...DEFAULT_PROXY_FILTERS });
  }, []);

  if (loading) {
    return <LoadingState label={uiText("加载中...", "Loading...", "読み込み中...")} />;
  }

  if (loadError) {
    return (
      <ErrorState
        title={uiText("加载日志失败", "Failed to load logs", "ログの読み込みに失敗しました")}
        message={loadError}
        retryLabel={uiText("重试", "Retry", "再試行")}
        onRetry={() => {
          void load({ force: true });
        }}
      />
    );
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{uiText("操作日志", "Activity Logs", "アクティビティログ")}</h2>
          <p className="page-subtitle">
            {uiText(
              "MCP 服务器活动、Provider 代理请求记录与模型价格配置",
              "MCP activity, provider proxy request records, and model pricing",
              "MCP の活動、Provider プロキシのリクエスト記録、モデル単価設定",
            )}
          </p>
        </div>
        <button className="btn btn-secondary btn-sm" onClick={handleRefresh}>
          {refreshing ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
          {uiText("刷新", "Refresh", "更新")}
        </button>
      </div>

      <div className="section-card" style={{ display: "flex", flexDirection: "column", marginBottom: 20 }}>
        <div
          style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 14 }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <DollarSign size={14} style={{ color: "var(--text-secondary)" }} />
            <span
              style={{
                fontSize: 12,
                fontWeight: 600,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: "0.04em",
              }}
            >
              {uiText("模型定价", "Model Pricing", "モデル単価")}
            </span>
            <span className="badge badge-muted" style={{ fontSize: 10 }}>
              {modelPricingRows.length}
            </span>
          </div>
          {editingModelId && (
            <button className="btn btn-ghost btn-xs" onClick={resetPricingDraft} style={{ gap: 4 }}>
              <X size={12} />
              {uiText("取消编辑", "Cancel Edit", "編集をやめる")}
            </button>
          )}
        </div>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(170px, 1fr))",
            gap: 10,
            marginBottom: 12,
          }}
        >
          <input
            className="input"
            value={pricingDraft.model_id}
            onChange={(event) => setPricingDraft((current) => ({ ...current, model_id: event.target.value }))}
            placeholder={uiText("模型 ID", "Model ID", "モデル ID")}
          />
          <input
            className="input"
            value={pricingDraft.input_cost_per_million}
            onChange={(event) =>
              setPricingDraft((current) => ({ ...current, input_cost_per_million: event.target.value }))
            }
            placeholder={uiText("输入 / 百万 tokens", "Input / million tokens", "入力 / 100万 tokens")}
          />
          <input
            className="input"
            value={pricingDraft.output_cost_per_million}
            onChange={(event) =>
              setPricingDraft((current) => ({ ...current, output_cost_per_million: event.target.value }))
            }
            placeholder={uiText("输出 / 百万 tokens", "Output / million tokens", "出力 / 100万 tokens")}
          />
          <input
            className="input"
            value={pricingDraft.cache_read_cost_per_million}
            onChange={(event) =>
              setPricingDraft((current) => ({ ...current, cache_read_cost_per_million: event.target.value }))
            }
            placeholder={uiText("缓存读 / 百万", "Cache read / million", "キャッシュ読込 / 100万")}
          />
          <input
            className="input"
            value={pricingDraft.cache_write_cost_per_million}
            onChange={(event) =>
              setPricingDraft((current) => ({ ...current, cache_write_cost_per_million: event.target.value }))
            }
            placeholder={uiText("缓存写 / 百万", "Cache write / million", "キャッシュ作成 / 100万")}
          />
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 12,
            marginBottom: 12,
            flexWrap: "wrap",
          }}
        >
          <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
            {uiText(
              "费用会按响应模型优先、请求模型兜底进行匹配；支持 `models/` 前缀、供应商前缀和 `@low` 这类后缀的归一化匹配。",
              "Costs match by response model first, then request model fallback. Normalization handles `models/` prefixes, provider prefixes, and suffixes like `@low`.",
              "費用はレスポンスモデル優先・リクエストモデル補完で照合します。`models/` 接頭辞、Provider 接頭辞、`@low` のような接尾辞も正規化して一致させます。",
            )}
          </span>
          <button
            className="btn btn-primary btn-sm"
            onClick={() => void handleSavePricing()}
            disabled={savingPricing}
            style={{ gap: 6 }}
          >
            {savingPricing ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Save size={14} />}
            {editingModelId
              ? uiText("更新定价", "Update Pricing", "単価を更新")
              : uiText("新增定价", "Add Pricing", "単価を追加")}
          </button>
        </div>

        {modelPricingRows.length === 0 ? (
          <div style={{ padding: "12px 0", color: "var(--text-muted)", fontSize: 13, textAlign: "center" }}>
            {uiText(
              "还没有模型定价。添加后代理日志会开始计算成本。",
              "No model pricing yet. Add entries to start calculating proxy cost.",
              "モデル単価はまだありません。追加するとプロキシのコスト計算が有効になります。",
            )}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 2, maxHeight: 220, overflowY: "auto" }}>
            {modelPricingRows.map((item) => (
              <ModelPricingListItem
                key={item.model_id}
                item={item as ModelPricingListItemRow}
                deleting={deletingPricingId === item.model_id}
                editTitle={uiText("编辑定价", "Edit pricing", "単価を編集")}
                deleteTitle={uiText("删除定价", "Delete pricing", "単価を削除")}
                onEdit={startEditingPricing}
                onDelete={handleDeletePricing}
              />
            ))}
          </div>
        )}
      </div>

      <div className="section-card" style={{ display: "flex", flexDirection: "column", marginBottom: 20 }}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: 14,
            gap: 12,
            flexWrap: "wrap",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <Activity size={14} style={{ color: "var(--text-secondary)" }} />
            <span
              style={{
                fontSize: 12,
                fontWeight: 600,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: "0.04em",
              }}
            >
              {uiText("Provider 代理请求", "Provider Proxy Requests", "Provider プロキシリクエスト")}
            </span>
            <span className="badge badge-muted" style={{ fontSize: 10 }}>
              {proxySummary?.total_requests || 0}
            </span>
          </div>
          {proxySummary && proxySummary.total_requests > 0 && (
            <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
              {uiText(
                `成功率 ${proxySummary.success_rate.toFixed(1)}%`,
                `Success ${proxySummary.success_rate.toFixed(1)}%`,
                `成功率 ${proxySummary.success_rate.toFixed(1)}%`,
              )}
            </span>
          )}
          <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
            <label
              style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 12, color: "var(--text-secondary)" }}
            >
              <input
                type="checkbox"
                checked={autoRefreshEnabled}
                onChange={(event) => setAutoRefreshEnabled(event.target.checked)}
              />
              {uiText("自动刷新", "Auto Refresh", "自動更新")}
            </label>
            <select
              className="input"
              value={String(autoRefreshInterval)}
              onChange={(event) =>
                setAutoRefreshInterval(Number(event.target.value) as (typeof AUTO_REFRESH_INTERVALS)[number])
              }
              disabled={!autoRefreshEnabled}
              style={{ width: 96, fontSize: 12 }}
            >
              {AUTO_REFRESH_INTERVALS.map((seconds) => (
                <option key={seconds} value={seconds}>
                  {seconds}s
                </option>
              ))}
            </select>
            {autoRefreshEnabled && (
              <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                {refreshing
                  ? uiText("正在拉取增量日志…", "Fetching incremental logs...", "増分ログを取得中...")
                  : uiText(
                      "仅增量追加新请求，不重置滚动位置",
                      "Appends new requests without resetting scroll",
                      "スクロール位置を保ったまま新規リクエストだけを追加",
                    )}
              </span>
            )}
          </div>
        </div>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
            gap: 10,
            marginBottom: 12,
          }}
        >
          <select
            className="input"
            value={proxyFilters.toolId}
            onChange={(event) => setProxyFilters((current) => ({ ...current, toolId: event.target.value }))}
          >
            <option value="">{uiText("全部应用", "All Apps", "すべてのアプリ")}</option>
            {Object.entries(TOOL_LABELS).map(([toolId, label]) => (
              <option key={toolId} value={toolId}>
                {label}
              </option>
            ))}
          </select>
          <select
            className="input"
            value={proxyFilters.status}
            onChange={(event) => setProxyFilters((current) => ({ ...current, status: event.target.value }))}
          >
            <option value="all">{uiText("全部状态", "All Statuses", "すべての状態")}</option>
            <option value="success">{uiText("仅成功", "Success Only", "成功のみ")}</option>
            <option value="error">{uiText("仅错误", "Errors Only", "エラーのみ")}</option>
          </select>
          <select
            className="input"
            value={proxyFilters.streamMode}
            onChange={(event) => setProxyFilters((current) => ({ ...current, streamMode: event.target.value }))}
          >
            <option value="all">{uiText("全部模式", "All Modes", "すべてのモード")}</option>
            <option value="streaming">{uiText("仅流式", "Streaming Only", "ストリームのみ")}</option>
            <option value="non_streaming">{uiText("仅非流式", "Non-stream Only", "非ストリームのみ")}</option>
          </select>
          <input
            className="input"
            value={proxyFilters.providerQuery}
            onChange={(event) => setProxyFilters((current) => ({ ...current, providerQuery: event.target.value }))}
            placeholder={uiText("筛选 Provider 名称", "Filter provider", "Provider 名で絞り込み")}
          />
          <input
            className="input"
            value={proxyFilters.modelQuery}
            onChange={(event) => setProxyFilters((current) => ({ ...current, modelQuery: event.target.value }))}
            placeholder={uiText("筛选模型", "Filter model", "モデルで絞り込み")}
          />
        </div>

        <div
          style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12, gap: 12 }}
        >
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {hasActiveProxyFilters
              ? uiText(
                  `筛选后 ${proxyLogs.length} 条`,
                  `${proxyLogs.length} filtered rows`,
                  `${proxyLogs.length} 件に絞り込み`,
                )
              : uiText(`最近 ${proxyLogs.length} 条`, `${proxyLogs.length} recent rows`, `最近 ${proxyLogs.length} 件`)}
          </span>
          {hasActiveProxyFilters && (
            <button className="btn btn-ghost btn-xs" onClick={handleClearProxyFilters}>
              {uiText("清除筛选", "Clear Filters", "フィルター解除")}
            </button>
          )}
        </div>

        {!proxySummary || proxySummary.total_requests === 0 ? (
          <div style={{ padding: "16px 0", color: "var(--text-muted)", fontSize: 13, textAlign: "center" }}>
            {uiText("暂无代理请求记录", "No proxy requests yet", "まだプロキシのリクエスト記録がありません")}
          </div>
        ) : (
          <>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
                gap: 10,
                marginBottom: 14,
              }}
            >
              <UsageMetric
                label={uiText("累计成本", "Total Cost", "累計コスト")}
                value={formatUsd(proxySummary.total_cost_usd)}
              />
              <UsageMetric
                label={uiText("输入 Tokens", "Input Tokens", "入力 Tokens")}
                value={formatCount(proxySummary.total_input_tokens)}
              />
              <UsageMetric
                label={uiText("输出 Tokens", "Output Tokens", "出力 Tokens")}
                value={formatCount(proxySummary.total_output_tokens)}
              />
              <UsageMetric
                label={uiText("缓存读取", "Cache Read", "キャッシュ読込")}
                value={formatCount(proxySummary.total_cache_read_tokens)}
              />
              <UsageMetric
                label={uiText("缓存创建", "Cache Create", "キャッシュ作成")}
                value={formatCount(proxySummary.total_cache_creation_tokens)}
              />
            </div>

            <div
              ref={proxyListRef}
              style={{ display: "flex", flexDirection: "column", gap: 2, maxHeight: 360, overflowY: "auto" }}
            >
              {proxyLogs.map((item) => {
                const totalTokens = item.input_tokens + item.output_tokens;
                return (
                  <ProxyRequestRow
                    key={item.request_id}
                    item={item}
                    success={item.status_code >= 200 && item.status_code < 300}
                    toolLabel={TOOL_LABELS[item.tool_id] || item.tool_id}
                    modelLabel={item.response_model || item.request_model || "--"}
                    costLabel={formatUsd(item.total_cost_usd)}
                    tokenLabel={
                      totalTokens > 0
                        ? `${formatCount(totalTokens)} tok`
                        : item.is_streaming
                          ? uiText("流式", "stream", "ストリーム")
                          : "--"
                    }
                    latencyLabel={`${item.latency_ms}ms`}
                    createdAtLabel={formatDateTime(item.created_at)}
                  />
                );
              })}
            </div>
          </>
        )}
      </div>

      <div className="section-card" style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <Activity size={14} style={{ color: "var(--text-secondary)" }} />
            <span
              style={{
                fontSize: 12,
                fontWeight: 600,
                color: "var(--text-muted)",
                textTransform: "uppercase",
                letterSpacing: "0.04em",
              }}
            >
              {uiText(`${selectedDate} 的活动`, `Activity on ${selectedDate}`, `${selectedDate} の活動`)}
            </span>
            <span className="badge badge-muted" style={{ fontSize: 10 }}>
              {activities.length}
            </span>
          </div>
        </div>

        <div style={{ flex: 1, overflowY: "auto" }}>
          {activities.length === 0 ? (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                height: "100%",
                color: "var(--text-muted)",
                fontSize: 13,
              }}
            >
              {uiText("当日无活动记录", "No activity on this date", "この日の活動記録はありません")}
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {activities.map((item) => (
                <ActivityLogRow
                  key={item.id}
                  item={item}
                  recordedAtLabel={item.recorded_at.split("T")[1]?.slice(0, 8) || item.recorded_at}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function mergeProxyLogs(previous: ProxyRequestLogRow[], incoming: ProxyRequestLogRow[]) {
  const seen = new Set(incoming.map((item) => item.request_id));
  return [...incoming, ...previous.filter((item) => !seen.has(item.request_id))].slice(0, MAX_MERGED_PROXY_LOG_ROWS);
}

function filterProxyLogs(
  rows: ProxyRequestLogRow[],
  filters: {
    toolId: string;
    status: string;
    streamMode: string;
    providerQuery: string;
    modelQuery: string;
  },
) {
  const providerQuery = filters.providerQuery.trim().toLowerCase();
  const modelQuery = filters.modelQuery.trim().toLowerCase();

  return rows.filter((item) => {
    if (filters.toolId && item.tool_id !== filters.toolId) {
      return false;
    }

    if (providerQuery && !item.provider_name.toLowerCase().includes(providerQuery)) {
      return false;
    }

    if (
      modelQuery &&
      !(item.request_model || "").toLowerCase().includes(modelQuery) &&
      !(item.response_model || "").toLowerCase().includes(modelQuery)
    ) {
      return false;
    }

    if (filters.status === "success" && (item.status_code < 200 || item.status_code >= 300)) {
      return false;
    }

    if (filters.status === "error" && item.status_code >= 200 && item.status_code < 300) {
      return false;
    }

    if (filters.streamMode === "streaming" && !item.is_streaming) {
      return false;
    }

    if (filters.streamMode === "non_streaming" && item.is_streaming) {
      return false;
    }

    return true;
  });
}

function todayStr() {
  const date = new Date();
  return dateStr(date);
}

function dateStr(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

const UsageMetric = memo(function UsageMetric({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ padding: "10px 12px", borderRadius: 10, background: "var(--bg-input)" }}>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 6 }}>{label}</div>
      <div style={{ fontSize: 18, fontWeight: 700 }}>{value}</div>
    </div>
  );
});

function formatCount(value: number) {
  return Intl.NumberFormat("en-US").format(value);
}

function formatUsd(value: string) {
  const amount = Number(value);
  if (!Number.isFinite(amount)) return "$0.000000";
  return `$${amount.toFixed(amount >= 1 ? 4 : 6)}`;
}

function formatDateTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return `${dateStr(date)} ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}:${String(date.getSeconds()).padStart(2, "0")}`;
}
