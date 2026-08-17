import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle2, Gauge, Loader2, Plus, Trash2, XCircle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

interface EndpointLatency {
  url: string;
  latency: number | null;
  status: number | null;
  error: string | null;
}

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

function classify(result: EndpointLatency) {
  if (result.error || result.status === null) return "error";
  if (result.status >= 200 && result.status < 400) return "success";
  return "warning";
}

export default function ProfileEndpointProbePanel({
  locale,
  localeText,
  appId,
  providerId,
  baseUrl,
  candidates,
  customEndpoints,
  onCustomEndpointsChange,
}: ProfileEndpointProbePanelProps) {
  const [results, setResults] = useState<EndpointLatency[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [customUrls, setCustomUrls] = useState<string[]>(customEndpoints);
  const [customInput, setCustomInput] = useState("");
  const [customError, setCustomError] = useState<string | null>(null);

  const normalize = (value: string) => value.trim().replace(/\/+$/, "");
  const updateCustomUrls = useCallback(
    (next: string[]) => {
      const unique = Array.from(new Set(next.map(normalize).filter(Boolean)));
      setCustomUrls(unique);
      onCustomEndpointsChange?.(unique);
    },
    [onCustomEndpointsChange],
  );

  useEffect(() => {
    if (!providerId) {
      const next = Array.from(new Set(customEndpoints.map(normalize).filter(Boolean)));
      setCustomUrls((current) => (current.join("\n") === next.join("\n") ? current : next));
      return;
    }
    let cancelled = false;
    void invoke<unknown[]>("get_custom_endpoints", { app: appId, providerId })
      .then((items) => {
        if (cancelled) return;
        const urls = items
          .map((item) => (typeof item === "string" ? item : (item as { url?: unknown })?.url))
          .filter((item): item is string => typeof item === "string")
          .map(normalize);
        setCustomUrls(Array.from(new Set(urls.filter(Boolean))));
      })
      .catch((reason) => {
        if (!cancelled) setCustomError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [appId, customEndpoints, providerId]);
  const entries = useMemo(() => {
    const all = [baseUrl, ...candidates.split(/[,\n]/), ...customUrls];
    const valid: string[] = [];
    for (const raw of all) {
      const value = raw.trim().replace(/\/$/, "");
      if (!value || valid.includes(value)) continue;
      try {
        const parsed = new URL(value);
        if (parsed.protocol === "http:" || parsed.protocol === "https:") valid.push(value);
      } catch {
        // Ignore incomplete draft values; the editor will still preserve them for saving.
      }
    }
    return valid.slice(0, 64);
  }, [baseUrl, candidates, customUrls]);

  const addCustomEndpoint = async () => {
    const value = normalize(customInput);
    try {
      const parsed = new URL(value);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:")
        throw new Error("Only HTTP(S) URLs are supported");
      if (!value || customUrls.includes(value)) throw new Error("Endpoint already exists");
      if (providerId) {
        await invoke("add_custom_endpoint", { app: appId, providerId, url: value });
      }
      updateCustomUrls([...customUrls, value]);
      setCustomInput("");
      setCustomError(null);
    } catch (reason) {
      setCustomError(String(reason));
    }
  };

  const removeCustomEndpoint = async (value: string) => {
    try {
      if (providerId) {
        await invoke("remove_custom_endpoint", { app: appId, providerId, url: value });
      }
      updateCustomUrls(customUrls.filter((item) => item !== value));
    } catch (reason) {
      setCustomError(String(reason));
    }
  };

  const runProbe = async () => {
    if (entries.length === 0) return;
    setRunning(true);
    setError(null);
    try {
      const next = await invoke<EndpointLatency[]>("test_api_endpoints", {
        urls: entries,
        timeoutSecs: 10,
      });
      setResults(
        [...next].sort((a, b) => (a.latency ?? Number.MAX_SAFE_INTEGER) - (b.latency ?? Number.MAX_SAFE_INTEGER)),
      );
    } catch (reason) {
      setResults([]);
      setError(String(reason));
    } finally {
      setRunning(false);
    }
  };

  return (
    <div
      style={{
        border: "1px solid var(--border-subtle)",
        borderRadius: 7,
        padding: 12,
        background: "var(--bg-secondary)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 7, marginBottom: 6, fontSize: 12, fontWeight: 600 }}>
        <Gauge size={14} style={{ color: "var(--accent)" }} />
        {localeText("端点测速", "Endpoint probe", "エンドポイント測定")}
      </div>
      <div style={{ color: "var(--text-muted)", fontSize: 11, lineHeight: 1.5, marginBottom: 10 }}>
        {localeText(
          "以 HEAD 请求测量主地址和候选地址，必要时自动回退 GET，不会发送模型请求。",
          "Measure the primary and candidate URLs with HEAD, falling back to GET when needed. No model request is sent.",
          "HEAD で主 URL と候補 URL を測定し、必要なら GET にフォールバックします。モデルリクエストは送信しません。",
        )}
      </div>
      <button
        type="button"
        className="btn btn-secondary btn-sm"
        onClick={() => void runProbe()}
        disabled={running || entries.length === 0}
      >
        {running ? <Loader2 size={14} className="spin" /> : <Gauge size={14} />}
        {running
          ? localeText("测速中…", "Probing…", "測定中…")
          : localeText("开始测速", "Probe endpoints", "エンドポイントを測定")}
      </button>
      <div style={{ display: "flex", gap: 7, marginTop: 10, alignItems: "center" }}>
        <input
          className="input"
          value={customInput}
          onChange={(event) => setCustomInput(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void addCustomEndpoint();
            }
          }}
          placeholder={localeText("添加自定义端点", "Add custom endpoint", "カスタムエンドポイントを追加")}
          style={{ minWidth: 0, flex: 1, fontSize: 12 }}
        />
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={() => void addCustomEndpoint()}
          disabled={!customInput.trim()}
        >
          <Plus size={14} /> {localeText("添加", "Add", "追加")}
        </button>
      </div>
      {customUrls.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 5, marginTop: 8 }}>
          {customUrls.map((url) => (
            <div key={url} style={{ display: "flex", alignItems: "center", gap: 7, fontSize: 11 }}>
              <span
                style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                title={url}
              >
                {url}
              </span>
              <button
                type="button"
                className="btn btn-ghost btn-icon-sm"
                onClick={() => void removeCustomEndpoint(url)}
                title={localeText("删除自定义端点", "Remove custom endpoint", "カスタムエンドポイントを削除")}
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
        </div>
      )}
      {customError && <div style={{ color: "var(--danger)", fontSize: 11, marginTop: 6 }}>{customError}</div>}
      {entries.length === 0 && (
        <span style={{ marginLeft: 8, color: "var(--text-muted)", fontSize: 11 }}>
          {localeText(
            "先填写有效的 HTTP(S) 地址",
            "Enter at least one valid HTTP(S) URL",
            "有効な HTTP(S) URL を入力してください",
          )}
        </span>
      )}
      {error && <div style={{ color: "var(--danger)", fontSize: 11, marginTop: 8 }}>{error}</div>}
      {results.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 5, marginTop: 10 }}>
          {results.map((result) => {
            const state = classify(result);
            return (
              <div
                key={result.url}
                style={{ display: "flex", alignItems: "center", gap: 7, fontSize: 11, minWidth: 0 }}
              >
                {state === "error" ? (
                  <XCircle size={13} style={{ color: "var(--danger)", flexShrink: 0 }} />
                ) : (
                  <CheckCircle2
                    size={13}
                    style={{ color: state === "success" ? "var(--success)" : "var(--warning)", flexShrink: 0 }}
                  />
                )}
                <span
                  style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1 }}
                  title={result.url}
                >
                  {result.url}
                </span>
                <span style={{ color: state === "error" ? "var(--danger)" : "var(--text-secondary)", flexShrink: 0 }}>
                  {result.error ?? `${result.status ?? "-"} · ${result.latency ?? "-"} ms`}
                </span>
              </div>
            );
          })}
        </div>
      )}
      {results.length > 1 && (
        <div style={{ color: "var(--text-muted)", fontSize: 10, marginTop: 8 }}>
          {locale === "zh"
            ? "结果按延迟从低到高排列"
            : locale === "ja"
              ? "結果は低遅延順に並んでいます"
              : "Results are sorted by latency"}
        </div>
      )}
    </div>
  );
}
