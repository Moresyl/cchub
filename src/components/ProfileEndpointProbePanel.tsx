import { useEffect, useId, useMemo, useState } from "react";
import { CheckCircle2, Gauge, Loader2, Plus, Trash2, TriangleAlert, XCircle } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { collectProbeEndpoints, normalizeEndpoint, useEndpointProbe } from "./profileEndpointProbe";

interface ProfileEndpointProbePanelProps {
  locale: string;
  localeText: (zhText: string, enText: string, jaText?: string) => string;
  appId: string;
  providerId?: string | null;
  baseUrl: string;
  candidates: string;
  customEndpoints: string[];
  onCustomEndpointsChange?: (urls: string[]) => void;
}

export default function ProfileEndpointProbePanel({
  localeText,
  appId,
  providerId,
  baseUrl,
  candidates,
  customEndpoints,
  onCustomEndpointsChange,
}: ProfileEndpointProbePanelProps) {
  const inputId = useId();
  const headingId = useId();
  const [customInput, setCustomInput] = useState("");
  const [customError, setCustomError] = useState<string | null>(null);
  const scope = JSON.stringify([appId, providerId ?? "new"]);
  const customUrls = [...new Set(customEndpoints.map(normalizeEndpoint).filter(Boolean))];
  const entries = useMemo(
    () => collectProbeEndpoints(baseUrl, candidates, customEndpoints),
    [baseUrl, candidates, customEndpoints],
  );
  const { results, running, error, run } = useEndpointProbe(scope, entries);

  useEffect(() => {
    setCustomInput("");
    setCustomError(null);
  }, [scope]);

  const addCustomEndpoint = () => {
    const value = normalizeEndpoint(customInput);
    if (!collectProbeEndpoints(value, "", []).length) {
      setCustomError(
        localeText(
          "请输入不含用户名和密码的 HTTP(S) 地址",
          "Enter an HTTP(S) URL without a username or password",
          "ユーザー名とパスワードを含まない HTTP(S) URL を入力してください",
        ),
      );
      return;
    }
    if (customUrls.includes(value)) {
      setCustomError(
        localeText("这个端点已经添加", "This endpoint has already been added", "このエンドポイントは追加済みです"),
      );
      return;
    }
    if (customUrls.length >= 128) {
      setCustomError(
        localeText(
          "最多添加 128 个自定义端点",
          "You can add up to 128 custom endpoints",
          "カスタムエンドポイントは最大 128 件です",
        ),
      );
      return;
    }
    onCustomEndpointsChange?.([...customUrls, value]);
    setCustomInput("");
    setCustomError(null);
  };

  return (
    <section aria-labelledby={headingId} className="min-w-0 rounded-lg border border-border bg-card p-3">
      <h3 id={headingId} className="mb-1 flex items-center gap-2 text-xs font-[590]">
        <Gauge size={14} aria-hidden="true" />
        {localeText("端点测速", "Endpoint probe", "エンドポイント測定")}
      </h3>
      <p className="mb-3 text-xs leading-relaxed text-muted-foreground">
        {localeText(
          "使用当前草稿地址发送 HEAD 请求，必要时回退 GET。结果只表示地址响应和延迟，不验证密钥或模型。",
          "Send HEAD requests to the current draft URLs, falling back to GET when needed. Results show URL responses and latency, not key or model validity.",
          "現在の下書き URL に HEAD を送信し、必要なら GET に切り替えます。URL の応答と遅延のみを測定し、キーやモデルは検証しません。",
        )}
      </p>
      <Button type="button" variant="outline" onClick={() => void run()} disabled={running || !entries.length}>
        {running ? <Loader2 size={14} className="animate-spin motion-reduce:animate-none" /> : <Gauge size={14} />}
        {running
          ? localeText("测速中…", "Probing…", "測定中…")
          : localeText("开始测速", "Probe endpoints", "エンドポイントを測定")}
      </Button>
      {!entries.length && (
        <p className="mt-2 text-xs text-muted-foreground">
          {localeText(
            "先填写有效的 HTTP(S) 地址",
            "Enter at least one valid HTTP(S) URL",
            "有効な HTTP(S) URL を入力してください",
          )}
        </p>
      )}
      <div className="mt-3">
        <label htmlFor={inputId} className="mb-1.5 block text-xs text-muted-foreground">
          {localeText("自定义端点", "Custom endpoint", "カスタムエンドポイント")}
        </label>
        <div className="flex items-center gap-2">
          <Input
            id={inputId}
            value={customInput}
            className="min-w-0 flex-1"
            disabled={!onCustomEndpointsChange}
            aria-invalid={!!customError}
            aria-describedby={customError ? `${inputId}-error` : undefined}
            onChange={(event) => {
              setCustomInput(event.target.value);
              setCustomError(null);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                event.preventDefault();
                addCustomEndpoint();
              }
            }}
            placeholder="https://api.example.com"
          />
          <Button
            type="button"
            variant="outline"
            onClick={addCustomEndpoint}
            disabled={!customInput.trim() || !onCustomEndpointsChange}
          >
            <Plus size={14} /> {localeText("添加", "Add", "追加")}
          </Button>
        </div>
        {customError && (
          <p id={`${inputId}-error`} role="alert" className="mt-1.5 text-xs text-[var(--danger)]">
            {customError}
          </p>
        )}
        <p className="mt-1.5 text-[11px] text-muted-foreground">
          {localeText(
            "端点修改随配置保存，取消编辑会放弃修改。每次最多测速 64 个地址。",
            "Endpoint changes are saved with the profile and discarded when editing is cancelled. Each probe checks up to 64 URLs.",
            "エンドポイントの変更は設定と一緒に保存され、編集をキャンセルすると破棄されます。1 回の測定は最大 64 URL です。",
          )}
        </p>
      </div>
      {!!customUrls.length && (
        <ul className="mt-2 grid min-w-0 gap-1">
          {customUrls.map((url) => (
            <li key={url} className="flex min-w-0 items-center gap-2 text-xs">
              <span className="min-w-0 flex-1 truncate" title={url}>
                {url}
              </span>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                disabled={!onCustomEndpointsChange}
                aria-label={localeText(`删除端点 ${url}`, `Remove endpoint ${url}`, `エンドポイント ${url} を削除`)}
                onClick={() => onCustomEndpointsChange?.(customUrls.filter((item) => item !== url))}
              >
                <Trash2 size={13} />
              </Button>
            </li>
          ))}
        </ul>
      )}
      {error && (
        <p role="alert" className="mt-2 break-words text-xs text-[var(--danger)]">
          {error}
        </p>
      )}
      {!!results.length && (
        <div className="mt-3 border-t border-border pt-3">
          <p role="status" className="mb-2 text-[11px] text-muted-foreground">
            {localeText(
              `已测量 ${results.length} 个地址，按延迟排序；密钥和模型尚未验证。`,
              `Measured ${results.length} URLs, sorted by latency; key and model validity have not been checked.`,
              `${results.length} 件の URL を遅延順に表示します。キーとモデルは未検証です。`,
            )}
          </p>
          <ul className="grid min-w-0 gap-2">
            {results.map((result) => {
              const failed = !!result.error || result.status === null;
              const successful = !failed && result.status! >= 200 && result.status! < 400;
              const Icon = failed ? XCircle : successful ? CheckCircle2 : TriangleAlert;
              return (
                <li key={result.url} className="flex min-w-0 flex-wrap items-start gap-x-2 gap-y-1 text-xs">
                  <Icon
                    size={14}
                    aria-hidden="true"
                    className="mt-0.5 shrink-0"
                    style={{ color: failed ? "var(--danger)" : successful ? "var(--success)" : "var(--warning)" }}
                  />
                  <span className="min-w-0 flex-[1_1_180px] break-all">{result.url}</span>
                  <span className="min-w-0 max-w-full break-words text-muted-foreground">
                    {result.error ?? `HTTP ${result.status ?? "—"} · ${result.latency ?? "—"} ms`}
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </section>
  );
}
