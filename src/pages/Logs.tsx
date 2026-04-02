import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, DollarSign, PencilLine, RefreshCw, Save, Trash2, X } from "lucide-react";
import { showToast } from "../components/Toast";
import { getLocale } from "../lib/i18n";

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

export default function Logs() {
  const [activities, setActivities] = useState<ActivityItem[]>([]);
  const [proxySummary, setProxySummary] = useState<ProxyUsageSummary | null>(null);
  const [proxyLogs, setProxyLogs] = useState<ProxyRequestLogRow[]>([]);
  const [modelPricingRows, setModelPricingRows] = useState<ModelPricingRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [proxyFilters, setProxyFilters] = useState(DEFAULT_PROXY_FILTERS);
  const [pricingDraft, setPricingDraft] = useState<ModelPricingDraft>(EMPTY_PRICING_DRAFT);
  const [editingModelId, setEditingModelId] = useState<string | null>(null);
  const [savingPricing, setSavingPricing] = useState(false);
  const [deletingPricingId, setDeletingPricingId] = useState<string | null>(null);
  const [selectedDate] = useState<string>(todayStr());
  const locale = getLocale();
  const uiText = (zhText: string, enText: string, jaText?: string) =>
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;

  async function load() {
    setLoading(true);
    try {
      const [acts, summary, filteredProxyLogs, pricing] = await Promise.all([
        invoke<ActivityItem[]>("get_activity_logs", { date: selectedDate }),
        invoke<ProxyUsageSummary>("get_proxy_usage_summary"),
        invoke<ProxyRequestLogRow[]>("search_proxy_request_logs", {
          filters: {
            limit: 80,
            tool_id: proxyFilters.toolId || null,
            provider_query: proxyFilters.providerQuery,
            model_query: proxyFilters.modelQuery,
            status: proxyFilters.status,
            stream_mode: proxyFilters.streamMode,
          },
        }),
        invoke<ModelPricingRow[]>("list_model_pricing"),
      ]);
      setActivities(acts);
      setProxySummary(summary);
      setProxyLogs(filteredProxyLogs);
      setModelPricingRows(pricing);
    } catch (error) {
      console.error(error);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, [selectedDate, proxyFilters]);

  const hasActiveProxyFilters =
    proxyFilters.toolId !== "" ||
    proxyFilters.status !== "all" ||
    proxyFilters.streamMode !== "all" ||
    proxyFilters.providerQuery.trim() !== "" ||
    proxyFilters.modelQuery.trim() !== "";

  async function handleSavePricing() {
    if (savingPricing) return;
    setSavingPricing(true);
    try {
      await invoke<ModelPricingRow>("save_model_pricing", { entry: pricingDraft });
      showToast("success", uiText("模型定价已保存", "Model pricing saved", "モデル単価を保存しました"));
      setPricingDraft(EMPTY_PRICING_DRAFT);
      setEditingModelId(null);
      await load();
    } catch (error) {
      console.error(error);
      showToast(
        "error",
        uiText(`保存模型定价失败: ${error}`, `Failed to save model pricing: ${error}`, `モデル単価の保存に失敗しました: ${error}`),
      );
    } finally {
      setSavingPricing(false);
    }
  }

  async function handleDeletePricing(modelId: string) {
    setDeletingPricingId(modelId);
    try {
      await invoke("delete_model_pricing", { modelId });
      if (editingModelId === modelId) {
        setPricingDraft(EMPTY_PRICING_DRAFT);
        setEditingModelId(null);
      }
      showToast("success", uiText("模型定价已删除", "Model pricing deleted", "モデル単価を削除しました"));
      await load();
    } catch (error) {
      console.error(error);
      showToast(
        "error",
        uiText(`删除模型定价失败: ${error}`, `Failed to delete model pricing: ${error}`, `モデル単価の削除に失敗しました: ${error}`),
      );
    } finally {
      setDeletingPricingId((current) => (current === modelId ? null : current));
    }
  }

  function startEditingPricing(row: ModelPricingRow) {
    setEditingModelId(row.model_id);
    setPricingDraft({
      model_id: row.model_id,
      input_cost_per_million: row.input_cost_per_million,
      output_cost_per_million: row.output_cost_per_million,
      cache_read_cost_per_million: row.cache_read_cost_per_million,
      cache_write_cost_per_million: row.cache_write_cost_per_million,
    });
  }

  function resetPricingDraft() {
    setPricingDraft(EMPTY_PRICING_DRAFT);
    setEditingModelId(null);
  }

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
          {uiText("加载中...", "Loading...", "読み込み中...")}
        </span>
      </div>
    );
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
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
        <button className="btn btn-secondary btn-sm" onClick={() => void load()}>
          <RefreshCw size={14} />
          {uiText("刷新", "Refresh", "更新")}
        </button>
      </div>

      <div className="section-card" style={{ display: "flex", flexDirection: "column", marginBottom: 20 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 14 }}>
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

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(170px, 1fr))", gap: 10, marginBottom: 12 }}>
          <input
            className="input"
            value={pricingDraft.model_id}
            onChange={(event) => setPricingDraft((current) => ({ ...current, model_id: event.target.value }))}
            placeholder={uiText("模型 ID", "Model ID", "モデル ID")}
          />
          <input
            className="input"
            value={pricingDraft.input_cost_per_million}
            onChange={(event) => setPricingDraft((current) => ({ ...current, input_cost_per_million: event.target.value }))}
            placeholder={uiText("输入 / 百万 tokens", "Input / million tokens", "入力 / 100万 tokens")}
          />
          <input
            className="input"
            value={pricingDraft.output_cost_per_million}
            onChange={(event) => setPricingDraft((current) => ({ ...current, output_cost_per_million: event.target.value }))}
            placeholder={uiText("输出 / 百万 tokens", "Output / million tokens", "出力 / 100万 tokens")}
          />
          <input
            className="input"
            value={pricingDraft.cache_read_cost_per_million}
            onChange={(event) => setPricingDraft((current) => ({ ...current, cache_read_cost_per_million: event.target.value }))}
            placeholder={uiText("缓存读 / 百万", "Cache read / million", "キャッシュ読込 / 100万")}
          />
          <input
            className="input"
            value={pricingDraft.cache_write_cost_per_million}
            onChange={(event) => setPricingDraft((current) => ({ ...current, cache_write_cost_per_million: event.target.value }))}
            placeholder={uiText("缓存写 / 百万", "Cache write / million", "キャッシュ作成 / 100万")}
          />
        </div>

        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 12, flexWrap: "wrap" }}>
          <span style={{ fontSize: 12, color: "var(--text-muted)" }}>
            {uiText(
              "费用会按响应模型优先、请求模型兜底进行匹配；支持 `models/` 前缀、供应商前缀和 `@low` 这类后缀的归一化匹配。",
              "Costs match by response model first, then request model fallback. Normalization handles `models/` prefixes, provider prefixes, and suffixes like `@low`.",
              "費用はレスポンスモデル優先・リクエストモデル補完で照合します。`models/` 接頭辞、Provider 接頭辞、`@low` のような接尾辞も正規化して一致させます。",
            )}
          </span>
          <button className="btn btn-primary btn-sm" onClick={() => void handleSavePricing()} disabled={savingPricing} style={{ gap: 6 }}>
            {savingPricing ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Save size={14} />}
            {editingModelId ? uiText("更新定价", "Update Pricing", "単価を更新") : uiText("新增定价", "Add Pricing", "単価を追加")}
          </button>
        </div>

        {modelPricingRows.length === 0 ? (
          <div style={{ padding: "12px 0", color: "var(--text-muted)", fontSize: 13, textAlign: "center" }}>
            {uiText("还没有模型定价。添加后代理日志会开始计算成本。", "No model pricing yet. Add entries to start calculating proxy cost.", "モデル単価はまだありません。追加するとプロキシのコスト計算が有効になります。")}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 2, maxHeight: 220, overflowY: "auto" }}>
            {modelPricingRows.map((item) => (
              <div key={item.model_id} className="list-row" style={{ padding: "10px 12px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
                <div style={{ minWidth: 0, flex: 1 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                    <span style={{ fontSize: 13, fontWeight: 600 }}>{item.model_id}</span>
                    {item.normalized_model_id !== item.model_id && (
                      <span className="badge badge-muted" style={{ fontSize: 10 }}>
                        {item.normalized_model_id}
                      </span>
                    )}
                  </div>
                  <div style={{ display: "flex", gap: 12, marginTop: 4, flexWrap: "wrap", fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                    <span>in {item.input_cost_per_million}</span>
                    <span>out {item.output_cost_per_million}</span>
                    <span>cache-r {item.cache_read_cost_per_million}</span>
                    <span>cache-w {item.cache_write_cost_per_million}</span>
                  </div>
                </div>
                <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                  <button className="btn btn-secondary btn-icon-sm" onClick={() => startEditingPricing(item)} title={uiText("编辑定价", "Edit pricing", "単価を編集")}>
                    <PencilLine size={14} />
                  </button>
                  <button
                    className="btn btn-danger-ghost btn-icon-sm"
                    onClick={() => void handleDeletePricing(item.model_id)}
                    disabled={deletingPricingId === item.model_id}
                    title={uiText("删除定价", "Delete pricing", "単価を削除")}
                  >
                    {deletingPricingId === item.model_id ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Trash2 size={14} />}
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="section-card" style={{ display: "flex", flexDirection: "column", marginBottom: 20 }}>
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
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))", gap: 10, marginBottom: 12 }}>
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

        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12, gap: 12 }}>
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {hasActiveProxyFilters
              ? uiText(`筛选后 ${proxyLogs.length} 条`, `${proxyLogs.length} filtered rows`, `${proxyLogs.length} 件に絞り込み`)
              : uiText(`最近 ${proxyLogs.length} 条`, `${proxyLogs.length} recent rows`, `最近 ${proxyLogs.length} 件`)}
          </span>
          {hasActiveProxyFilters && (
            <button className="btn btn-ghost btn-xs" onClick={() => setProxyFilters({ ...DEFAULT_PROXY_FILTERS })}>
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
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))", gap: 10, marginBottom: 14 }}>
              <UsageMetric label={uiText("累计成本", "Total Cost", "累計コスト")} value={formatUsd(proxySummary.total_cost_usd)} />
              <UsageMetric label={uiText("输入 Tokens", "Input Tokens", "入力 Tokens")} value={formatCount(proxySummary.total_input_tokens)} />
              <UsageMetric label={uiText("输出 Tokens", "Output Tokens", "出力 Tokens")} value={formatCount(proxySummary.total_output_tokens)} />
              <UsageMetric label={uiText("缓存读取", "Cache Read", "キャッシュ読込")} value={formatCount(proxySummary.total_cache_read_tokens)} />
              <UsageMetric label={uiText("缓存创建", "Cache Create", "キャッシュ作成")} value={formatCount(proxySummary.total_cache_creation_tokens)} />
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: 2, maxHeight: 360, overflowY: "auto" }}>
              {proxyLogs.map((item) => {
                const toolLabel = TOOL_LABELS[item.tool_id] || item.tool_id;
                const model = item.response_model || item.request_model || "--";
                const success = item.status_code >= 200 && item.status_code < 300;
                return (
                  <div
                    key={item.request_id}
                    className="list-row"
                    style={{ padding: "10px 12px", display: "flex", flexDirection: "column", alignItems: "stretch", gap: 6 }}
                  >
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
                      <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0, flex: 1 }}>
                        <span className={`dot ${success ? "dot-active" : "dot-error"}`} />
                        <span className="badge badge-muted" style={{ fontSize: 10 }}>
                          {toolLabel}
                        </span>
                        <span
                          style={{
                            fontSize: 13,
                            fontWeight: 500,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {item.provider_name}
                        </span>
                        <span
                          style={{
                            fontSize: 12,
                            color: "var(--text-muted)",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {model}
                        </span>
                      </div>
                      <div style={{ display: "flex", alignItems: "center", gap: 12, flexShrink: 0 }}>
                        <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                          {formatUsd(item.total_cost_usd)}
                        </span>
                        <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                          {item.input_tokens + item.output_tokens > 0
                            ? `${formatCount(item.input_tokens + item.output_tokens)} tok`
                            : item.is_streaming
                              ? uiText("流式", "stream", "ストリーム")
                              : "--"}
                        </span>
                        <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                          {item.latency_ms}ms
                        </span>
                        <span className={`badge ${success ? "badge-success" : "badge-danger"}`} style={{ fontSize: 10 }}>
                          {item.status_code}
                        </span>
                      </div>
                    </div>

                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
                      <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                        {item.request_id}
                      </span>
                      <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{formatDateTime(item.created_at)}</span>
                    </div>

                    {item.error_message && (
                      <div style={{ fontSize: 11, color: "var(--danger)", lineHeight: 1.5 }}>
                        {item.error_message}
                      </div>
                    )}
                  </div>
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
            <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--text-muted)", fontSize: 13 }}>
              {uiText("当日无活动记录", "No activity on this date", "この日の活動記録はありません")}
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {activities.map((item) => (
                <div key={item.id} className="list-row" style={{ padding: "10px 12px" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 10, flex: 1, minWidth: 0 }}>
                    <span className={`dot ${item.status === "success" ? "dot-active" : item.status === "error" ? "dot-error" : "dot-disabled"}`} />
                    <span style={{ fontSize: 13, fontWeight: 500 }}>{item.server_name}</span>
                    <span className="badge badge-muted" style={{ fontSize: 10 }}>
                      {item.request_type}
                    </span>
                  </div>
                  <div style={{ display: "flex", alignItems: "center", gap: 12, flexShrink: 0 }}>
                    {item.latency_ms != null && (
                      <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                        {item.latency_ms}ms
                      </span>
                    )}
                    <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                      {item.recorded_at.split("T")[1]?.slice(0, 8) || item.recorded_at}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function todayStr() {
  const date = new Date();
  return dateStr(date);
}

function dateStr(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function UsageMetric({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ padding: "10px 12px", borderRadius: 10, background: "var(--bg-input)" }}>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 6 }}>{label}</div>
      <div style={{ fontSize: 18, fontWeight: 700 }}>{value}</div>
    </div>
  );
}

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
