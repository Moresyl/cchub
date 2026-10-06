import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, RefreshCw } from "lucide-react";
import { Button } from "../ui/button";
import { Checkbox } from "../ui/checkbox";
import { Input } from "../ui/input";
import { SimpleSelect } from "../ui/simple-select";
import ErrorState from "../states/ErrorState";
import LoadingState from "../states/LoadingState";
import { isSelected, type CatalogEntry, type SyncConfig, type UiText } from "./types";

const PAGE_SIZE = 24;
function price(value: number) {
  return value.toFixed(6).replace(/0+$/, "").replace(/\.$/, "") || "0";
}
function ModelRow({
  entry,
  config,
  disabled,
  onSelect,
  text,
}: {
  entry: CatalogEntry;
  config: SyncConfig;
  disabled: boolean;
  onSelect: (entry: CatalogEntry, checked: boolean) => void;
  text: UiText;
}) {
  const id = useId();
  return (
    <div className="pricing-model-row" data-selected={isSelected(entry, config)}>
      <Checkbox
        id={id}
        aria-labelledby={`${id}-name`}
        aria-describedby={`${id}-details ${id}-price`}
        checked={isSelected(entry, config)}
        disabled={disabled}
        onCheckedChange={(checked) => onSelect(entry, checked === true)}
      />
      <div className="pricing-model-copy">
        <label id={`${id}-name`} htmlFor={id} className="pricing-model-name">
          {entry.modelName}
        </label>
        {entry.isCommon && <span className="badge badge-muted">{text("常用", "Common", "共通")}</span>}
        <div id={`${id}-details`} className="pricing-model-details">
          {entry.providerName} · {entry.modelId}
        </div>
        <div id={`${id}-price`} className="pricing-model-prices">
          <span>
            {text("输入", "Input", "入力")} ${price(entry.input)}
          </span>
          <span>
            {text("输出", "Output", "出力")} ${price(entry.output)}
          </span>
          {(entry.cacheRead > 0 || entry.cacheWrite > 0) && (
            <>
              <span>
                {text("缓存读取", "Cache read", "キャッシュ読取")} ${price(entry.cacheRead)}
              </span>
              <span>
                {text("缓存写入", "Cache write", "キャッシュ書込")} ${price(entry.cacheWrite)}
              </span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default function ModelPicker({
  config,
  disabled,
  onSelect,
  text,
}: {
  config: SyncConfig;
  disabled: boolean;
  onSelect: (entry: CatalogEntry, checked: boolean) => void;
  text: UiText;
}) {
  const [catalog, setCatalog] = useState<CatalogEntry[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [failed, setFailed] = useState(false);
  const [search, setSearch] = useState("");
  const [provider, setProvider] = useState("all");
  const [page, setPage] = useState(0);
  const owner = useRef({ mounted: false, busy: false, request: 0 });
  const list = useRef<HTMLDivElement>(null);
  const searchId = useId();
  const load = useCallback(async () => {
    const active = owner.current;
    if (!active.mounted || active.busy) return;
    active.busy = true;
    const request = ++active.request;
    setLoading(true);
    setFailed(false);
    try {
      const next = await invoke<CatalogEntry[]>("get_models_dev_catalog");
      if (
        !Array.isArray(next) ||
        next.some(
          (entry) =>
            !entry ||
            typeof entry.key !== "string" ||
            typeof entry.modelName !== "string" ||
            typeof entry.modelId !== "string" ||
            typeof entry.providerId !== "string" ||
            typeof entry.providerName !== "string" ||
            typeof entry.isCommon !== "boolean" ||
            [entry.input, entry.output, entry.cacheRead, entry.cacheWrite].some(
              (value) => !Number.isFinite(value) || value < 0,
            ),
        )
      )
        throw new Error("Invalid catalog");
      if (active.mounted && active.request === request) {
        setCatalog(next);
        setPage(0);
      }
    } catch {
      if (active.mounted && active.request === request) setFailed(true);
    } finally {
      if (active.mounted && active.request === request) {
        active.busy = false;
        setLoading(false);
      }
    }
  }, []);
  useEffect(() => {
    const active = owner.current;
    active.mounted = true;
    void load();
    return () => {
      active.mounted = false;
      active.busy = false;
      ++active.request;
    };
  }, [load]);
  const providers = useMemo(
    () =>
      Array.from(new Map((catalog ?? []).map((entry) => [entry.providerId, entry.providerName]))).sort((a, b) =>
        a[1].localeCompare(b[1]),
      ),
    [catalog],
  );
  const filtered = useMemo(() => {
    const query = search.trim().toLowerCase();
    return (catalog ?? []).filter(
      (entry) =>
        (provider === "all" || entry.providerId === provider) &&
        (!query || `${entry.modelName} ${entry.modelId} ${entry.providerName}`.toLowerCase().includes(query)),
    );
  }, [catalog, provider, search]);
  const pages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const currentPage = Math.min(page, pages - 1);
  const visible = filtered.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE);
  const changePage = (next: number) => {
    setPage(next);
    if (list.current) list.current.scrollTop = 0;
  };
  const blocked = disabled || loading;
  return (
    <div className="pricing-picker">
      <div className="pricing-picker-filters">
        <div className="pricing-search">
          <label htmlFor={searchId}>
            {text("搜索模型或供应商", "Search models or providers", "モデル・Provider を検索")}
          </label>
          <Input
            id={searchId}
            value={search}
            disabled={disabled}
            onChange={(event) => {
              setSearch(event.target.value);
              changePage(0);
            }}
            placeholder={text("名称或模型 ID", "Name or model ID", "名前・モデル ID")}
          />
        </div>
        <div className="pricing-provider">
          <span>{text("供应商", "Provider", "Provider")}</span>
          <SimpleSelect
            value={provider}
            onValueChange={(value) => {
              setProvider(value);
              changePage(0);
            }}
            disabled={disabled}
            ariaLabel={text("供应商", "Provider", "Provider")}
            options={[
              { value: "all", label: text("全部供应商", "All providers", "すべての Provider") },
              ...providers.map(([value, label]) => ({ value, label })),
            ]}
          />
        </div>
        <Button type="button" variant="secondary" disabled={blocked} onClick={() => void load()}>
          <RefreshCw size={14} />
          {text("更新目录", "Refresh catalog", "カタログを更新")}
        </Button>
      </div>
      <p className="pricing-help">
        {text(
          "价格单位：美元 / 100 万 tokens。勾选仅修改草稿，保存后生效。",
          "Prices are USD per million tokens. Selections take effect after saving.",
          "価格は100万トークンあたり米ドル。選択は保存後に適用されます。",
        )}
      </p>
      {failed && (
        <ErrorState
          title={text("模型目录读取失败", "Could not load the catalog", "カタログを読み込めませんでした")}
          message={text(
            "已有选择已保留。检查网络后重试。",
            "Your selections are retained. Check the connection and retry.",
            "選択は保持されています。接続を確認して再試行してください。",
          )}
          retryLabel={text("重试", "Retry", "再試行")}
          onRetry={blocked ? undefined : () => void load()}
        />
      )}
      {loading && (
        <div role="status">
          <LoadingState label={text("正在读取模型目录", "Loading catalog", "カタログを読込中")} />
        </div>
      )}
      {catalog && (
        <>
          <div
            ref={list}
            className="pricing-model-list"
            aria-label={text("模型目录", "Model catalog", "モデルカタログ")}
            tabIndex={0}
            aria-busy={loading}
          >
            {visible.map((entry) => (
              <ModelRow
                key={entry.key}
                entry={entry}
                config={config}
                disabled={blocked}
                onSelect={onSelect}
                text={text}
              />
            ))}
            {!visible.length && (
              <div className="empty-state">
                <div className="state-copy">
                  {text("没有匹配模型", "No matching models", "一致するモデルがありません")}
                </div>
                {(search || provider !== "all") && (
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={blocked}
                    onClick={() => {
                      setSearch("");
                      setProvider("all");
                      changePage(0);
                    }}
                  >
                    {text("清除筛选", "Clear filters", "絞り込みを解除")}
                  </Button>
                )}
              </div>
            )}
          </div>
          <nav
            className="pricing-pagination"
            aria-label={text("模型目录分页", "Catalog pagination", "カタログのページ")}
          >
            <span role="status">
              {text(
                `显示 ${filtered.length ? currentPage * PAGE_SIZE + 1 : 0}–${Math.min((currentPage + 1) * PAGE_SIZE, filtered.length)} / ${filtered.length}`,
                `Showing ${filtered.length ? currentPage * PAGE_SIZE + 1 : 0}–${Math.min((currentPage + 1) * PAGE_SIZE, filtered.length)} / ${filtered.length}`,
                `${filtered.length} 件中 ${visible.length} 件を表示`,
              )}
            </span>
            <div>
              <Button
                type="button"
                variant="secondary"
                size="icon"
                aria-label={text("上一页", "Previous page", "前のページ")}
                disabled={blocked || currentPage === 0}
                onClick={() => changePage(currentPage - 1)}
              >
                <ChevronLeft size={16} />
              </Button>
              <span>
                {currentPage + 1} / {pages}
              </span>
              <Button
                type="button"
                variant="secondary"
                size="icon"
                aria-label={text("下一页", "Next page", "次のページ")}
                disabled={blocked || currentPage + 1 >= pages}
                onClick={() => changePage(currentPage + 1)}
              >
                <ChevronRight size={16} />
              </Button>
            </div>
          </nav>
        </>
      )}
    </div>
  );
}
