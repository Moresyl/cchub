import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, ChevronUp, CloudDownload, RefreshCw, Save } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";

interface SyncConfig {
  autoSyncEnabled: boolean;
  includeCommonModels: boolean;
  selectedModelKeys: string[];
  excludedCommonModelKeys: string[];
  lastSyncAt: number | null;
  lastSyncError: string | null;
}

interface SyncState {
  config: SyncConfig;
  configPath: string;
}

interface CatalogEntry {
  key: string;
  providerId: string;
  providerName: string;
  modelId: string;
  modelName: string;
  releaseDate: string;
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
}

interface SyncResult {
  skipped: boolean;
  selected: number;
  imported: number;
  changed: number;
  syncedAt: number | null;
}

const MAX_VISIBLE_ENTRIES = 180;

function isCommonEntry(entry: CatalogEntry) {
  const prefixes: Record<string, string[]> = {
    anthropic: ["claude-"],
    openai: ["gpt-", "o1-", "o3-", "o4-"],
    google: ["gemini-"],
    xai: ["grok-"],
    deepseek: ["deepseek-"],
    alibaba: ["qwen"],
    xiaomi: ["mimo-"],
    longcat: ["longcat-"],
    moonshotai: ["kimi-"],
    "minimax-cn": ["minimax-m"],
    zai: ["glm-"],
  };
  const modelId = entry.modelId.toLowerCase();
  return (prefixes[entry.providerId] ?? []).some((prefix) => modelId.startsWith(prefix));
}

function formatPrice(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0";
  return value.toFixed(6).replace(/0+$/, "").replace(/\.$/, "") || "0";
}

export default function ModelsDevSyncPanel() {
  const locale = getLocale();
  const uiText = useCallback(
    (zh: string, en: string, ja?: string) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en),
    [locale],
  );
  const [state, setState] = useState<SyncState | null>(null);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [excludedCommon, setExcludedCommon] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [provider, setProvider] = useState("all");
  const [showPicker, setShowPicker] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadingCatalog, setLoadingCatalog] = useState(false);
  const [busy, setBusy] = useState(false);

  const loadState = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SyncState>("get_models_dev_sync_config");
      setState(next);
      setSelected(new Set(next.config.selectedModelKeys));
      setExcludedCommon(new Set(next.config.excludedCommonModelKeys));
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadState();
  }, [loadState]);

  const openPicker = useCallback(async () => {
    setShowPicker((open) => !open);
    if (catalog.length > 0) return;
    setLoadingCatalog(true);
    try {
      setCatalog(await invoke<CatalogEntry[]>("get_models_dev_catalog"));
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setLoadingCatalog(false);
    }
  }, [catalog.length]);

  const providers = useMemo(
    () =>
      Array.from(new Map(catalog.map((entry) => [entry.providerId, entry.providerName]))).sort((left, right) =>
        left[1].localeCompare(right[1]),
      ),
    [catalog],
  );
  const filtered = useMemo(() => {
    const query = search.trim().toLowerCase();
    return catalog
      .filter((entry) => provider === "all" || entry.providerId === provider)
      .filter(
        (entry) => !query || `${entry.modelId} ${entry.modelName} ${entry.providerName}`.toLowerCase().includes(query),
      );
  }, [catalog, provider, search]);
  const visible = filtered.slice(0, MAX_VISIBLE_ENTRIES);
  const commonKeys = useMemo(() => new Set(catalog.filter(isCommonEntry).map((entry) => entry.key)), [catalog]);

  const saveConfig = useCallback(
    async (nextConfig: SyncConfig) => {
      setBusy(true);
      try {
        await invoke("save_models_dev_sync_config", { config: nextConfig });
        setState((current) => (current ? { ...current, config: nextConfig } : current));
        showToast("success", uiText("同步设置已保存", "Sync settings saved", "同期設定を保存しました"));
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setBusy(false);
      }
    },
    [uiText],
  );

  const saveSelection = useCallback(async () => {
    if (!state) return;
    await saveConfig({
      ...state.config,
      selectedModelKeys: Array.from(selected).sort(),
      excludedCommonModelKeys: Array.from(excludedCommon).sort(),
    });
  }, [excludedCommon, saveConfig, selected, state]);

  const syncNow = useCallback(async () => {
    setBusy(true);
    try {
      const result = await invoke<SyncResult>("sync_models_dev_pricing", { force: true });
      await loadState();
      showToast(
        "success",
        uiText(
          `已同步 ${result.imported} 个模型，更新 ${result.changed} 项`,
          `Synced ${result.imported} models, changed ${result.changed}`,
          `モデル ${result.imported} 件を同期、${result.changed} 件を更新`,
        ),
      );
    } catch (error) {
      showToast("error", String(error));
      await loadState();
    } finally {
      setBusy(false);
    }
  }, [loadState, uiText]);

  if (loading)
    return (
      <div className="section-card">
        <div className="spinner" />
      </div>
    );
  if (!state) return null;

  const lastSync = state.config.lastSyncAt
    ? new Date(state.config.lastSyncAt).toLocaleString()
    : uiText("从未同步", "Never synced", "未同期");

  return (
    <div className="section-card" style={{ display: "grid", gap: 12 }}>
      <div
        style={{
          display: "flex",
          alignItems: "flex-start",
          justifyContent: "space-between",
          gap: 12,
          flexWrap: "wrap",
        }}
      >
        <div>
          <div className="section-card-title">
            <CloudDownload size={16} />
            {uiText("模型价格自动同步", "Automatic model pricing sync", "モデル価格の自動同期")}
          </div>
          <div style={{ color: "var(--text-muted)", fontSize: 11, marginTop: 5 }}>
            {uiText(
              "从公开模型目录更新本地价格，代理成本统计会自动使用最新配置。",
              "Refresh local pricing from the public model catalog for accurate proxy cost reports.",
              "公開モデルカタログから価格を更新し、プロキシのコスト集計を最新化します。",
            )}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <button
            className="btn btn-secondary btn-sm"
            type="button"
            onClick={() => void loadState()}
            disabled={busy}
            title={uiText("刷新状态", "Refresh status", "状態を更新")}
          >
            <RefreshCw size={14} />
          </button>
          <button className="btn btn-primary btn-sm" type="button" onClick={() => void syncNow()} disabled={busy}>
            <CloudDownload size={14} />
            {uiText("立即同步", "Sync now", "今すぐ同期")}
          </button>
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(190px, 1fr))", gap: 10 }}>
        <label className="toggle-row" style={{ alignItems: "flex-start" }}>
          <input
            type="checkbox"
            checked={state.config.autoSyncEnabled}
            disabled={busy}
            onChange={(event) => void saveConfig({ ...state.config, autoSyncEnabled: event.target.checked })}
          />
          <span>
            <strong>{uiText("启动时自动同步", "Sync on startup", "起動時に同期")}</strong>
            <small>{uiText("最多每 6 小时执行一次", "At most once every 6 hours", "最短 6 時間間隔")}</small>
          </span>
        </label>
        <label className="toggle-row" style={{ alignItems: "flex-start" }}>
          <input
            type="checkbox"
            checked={state.config.includeCommonModels}
            disabled={busy}
            onChange={(event) => void saveConfig({ ...state.config, includeCommonModels: event.target.checked })}
          />
          <span>
            <strong>{uiText("包含常用模型", "Include common models", "一般的なモデルを含める")}</strong>
            <small>
              {uiText("每个模型族保留最近版本", "Keep recent entries per model family", "各モデル系列の最近の版を保持")}
            </small>
          </span>
        </label>
      </div>

      <div style={{ display: "flex", gap: 12, flexWrap: "wrap", color: "var(--text-muted)", fontSize: 11 }}>
        <span>
          {uiText("上次同步", "Last sync", "最終同期")}: {lastSync}
        </span>
        <span style={{ overflow: "hidden", textOverflow: "ellipsis" }} title={state.configPath}>
          {uiText("配置", "Config", "設定")}: {state.configPath}
        </span>
      </div>
      {state.config.lastSyncError ? <div className="inline-error">{state.config.lastSyncError}</div> : null}

      <div
        style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10, flexWrap: "wrap" }}
      >
        <span style={{ fontSize: 12 }}>
          {uiText(`${selected.size} 个显式模型`, `${selected.size} explicit models`, `${selected.size} 件の明示モデル`)}
        </span>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="btn btn-secondary btn-sm"
            type="button"
            onClick={() => void openPicker()}
            disabled={busy || loadingCatalog}
          >
            {showPicker ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            {showPicker
              ? uiText("收起选择器", "Hide picker", "選択を閉じる")
              : uiText("选择模型", "Choose models", "モデルを選択")}
          </button>
          {showPicker ? (
            <button
              className="btn btn-primary btn-sm"
              type="button"
              onClick={() => void saveSelection()}
              disabled={busy}
            >
              <Save size={14} />
              {uiText("保存选择", "Save selection", "選択を保存")}
            </button>
          ) : null}
        </div>
      </div>

      {showPicker ? (
        <div style={{ display: "grid", gap: 10 }}>
          {loadingCatalog ? (
            <div className="empty-state">
              <div className="spinner" />
            </div>
          ) : (
            <>
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                <input
                  className="input"
                  style={{ flex: "1 1 240px" }}
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder={uiText("搜索模型或供应商", "Search models or providers", "モデル・Provider を検索")}
                />
                <select
                  className="input"
                  style={{ flex: "0 1 210px" }}
                  value={provider}
                  onChange={(event) => setProvider(event.target.value)}
                >
                  <option value="all">{uiText("全部供应商", "All providers", "すべての Provider")}</option>
                  {providers.map(([id, name]) => (
                    <option key={id} value={id}>
                      {name}
                    </option>
                  ))}
                </select>
              </div>
              <div
                style={{ display: "flex", justifyContent: "space-between", color: "var(--text-muted)", fontSize: 11 }}
              >
                <span>
                  {uiText(
                    `显示 ${visible.length} / ${filtered.length}`,
                    `Showing ${visible.length} / ${filtered.length}`,
                    `${visible.length} / ${filtered.length} 件を表示`,
                  )}
                </span>
                <span>{uiText("勾选后保存选择", "Save after selecting", "選択後に保存")}</span>
              </div>
              <div
                style={{ maxHeight: 360, overflowY: "auto", border: "1px solid var(--border-subtle)", borderRadius: 6 }}
              >
                {visible.map((entry) => {
                  const common = commonKeys.has(entry.key);
                  const checked =
                    selected.has(entry.key) ||
                    (state.config.includeCommonModels && common && !excludedCommon.has(entry.key));
                  return (
                    <label
                      key={entry.key}
                      style={{
                        display: "grid",
                        gridTemplateColumns: "20px minmax(0, 1fr) auto",
                        alignItems: "center",
                        gap: 8,
                        padding: "8px 10px",
                        borderBottom: "1px solid var(--border-subtle)",
                        background: checked ? "var(--accent-subtle)" : undefined,
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => {
                          if (common && state.config.includeCommonModels) {
                            setSelected((current) => {
                              const next = new Set(current);
                              next.delete(entry.key);
                              return next;
                            });
                            setExcludedCommon((current) => {
                              const next = new Set(current);
                              if (next.has(entry.key)) next.delete(entry.key);
                              else next.add(entry.key);
                              return next;
                            });
                          } else {
                            setSelected((current) => {
                              const next = new Set(current);
                              if (next.has(entry.key)) next.delete(entry.key);
                              else next.add(entry.key);
                              return next;
                            });
                          }
                        }}
                      />
                      <span style={{ minWidth: 0 }}>
                        <strong
                          style={{
                            display: "block",
                            fontSize: 12,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {entry.modelName}{" "}
                          {common ? (
                            <span className="badge badge-muted">{uiText("常用", "Common", "共通")}</span>
                          ) : null}
                        </strong>
                        <small
                          style={{
                            display: "block",
                            color: "var(--text-muted)",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {entry.providerName} · {entry.modelId}
                        </small>
                      </span>
                      <span style={{ fontSize: 10, color: "var(--text-muted)", whiteSpace: "nowrap" }}>
                        ${formatPrice(entry.input)} / ${formatPrice(entry.output)}
                      </span>
                    </label>
                  );
                })}
                {visible.length === 0 ? (
                  <div className="empty-state">
                    <div className="state-copy">
                      {uiText("没有匹配模型", "No matching models", "一致するモデルがありません")}
                    </div>
                  </div>
                ) : null}
              </div>
            </>
          )}
        </div>
      ) : null}
    </div>
  );
}
