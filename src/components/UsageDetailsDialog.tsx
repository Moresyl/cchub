import { useCallback, useEffect, useMemo, useState } from "react";
import { Copy, Gauge, RefreshCw, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "./Toast";
import type { ConfigProfile } from "../pages/profiles/helpers";

interface UsageResult {
  success?: boolean;
  provider?: string;
  data?: unknown;
  error?: string;
}

interface UsageDetailsDialogProps {
  profile: ConfigProfile | null;
  locale: string;
  onClose: () => void;
}

function text(locale: string, zh: string, en: string, ja: string) {
  return locale === "zh" ? zh : locale === "ja" ? ja : en;
}

function formatValue(value: unknown) {
  if (value == null) return "—";
  if (typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

function formatJson(value: unknown) {
  try {
    return JSON.stringify(value ?? {}, null, 2);
  } catch {
    return String(value ?? "");
  }
}

export default function UsageDetailsDialog({ profile, locale, onClose }: UsageDetailsDialogProps) {
  const [result, setResult] = useState<UsageResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!profile) return;
    setLoading(true);
    setError(null);
    try {
      const next = await invoke<UsageResult>("queryProviderUsage", {
        providerId: profile.id,
        app: profile.tool_id,
      });
      setResult(next);
    } catch (reason) {
      setResult(null);
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  }, [profile]);

  useEffect(() => {
    if (!profile) return;
    void load();
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [load, onClose, profile]);

  const rows = useMemo(() => {
    const value = result?.data;
    if (Array.isArray(value)) return value.filter((item) => item && typeof item === "object");
    return value && typeof value === "object" ? [value] : [];
  }, [result?.data]);

  if (!profile) return null;

  const copyResult = async () => {
    try {
      await navigator.clipboard.writeText(formatJson(result));
      showToast("success", text(locale, "用量结果已复制", "Usage result copied", "使用量の結果をコピーしました"));
    } catch (reason) {
      showToast(
        "error",
        text(locale, `复制失败: ${reason}`, `Copy failed: ${reason}`, `コピーに失敗しました: ${reason}`),
      );
    }
  };

  return (
    <div className="confirm-overlay" onClick={onClose}>
      <div
        className="confirm-dialog animate-in"
        style={{
          width: "min(720px, calc(100vw - 32px))",
          maxHeight: "min(760px, calc(100vh - 48px))",
          overflow: "auto",
        }}
        onClick={(event) => event.stopPropagation()}
      >
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
          <div
            style={{
              width: 36,
              height: 36,
              borderRadius: 9,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              background: "var(--accent-subtle)",
              color: "var(--accent)",
              flexShrink: 0,
            }}
          >
            <Gauge size={18} />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h3 style={{ fontSize: 16, fontWeight: 650 }}>
              {text(locale, "Provider 用量", "Provider usage", "Provider 使用量")}
            </h3>
            <p
              style={{
                color: "var(--text-muted)",
                fontSize: 12,
                marginTop: 4,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {profile.name} · {profile.tool_id}
            </p>
          </div>
          <button
            className="btn btn-ghost btn-icon-sm"
            type="button"
            onClick={onClose}
            title={text(locale, "关闭", "Close", "閉じる")}
          >
            <X size={16} />
          </button>
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 8,
            marginTop: 18,
            flexWrap: "wrap",
          }}
        >
          <div
            style={{
              fontSize: 12,
              color: result?.success
                ? "var(--success)"
                : error || result?.error
                  ? "var(--danger)"
                  : "var(--text-muted)",
            }}
          >
            {loading
              ? text(locale, "正在查询…", "Querying…", "照会中…")
              : result?.success
                ? text(locale, "查询成功", "Query succeeded", "照会成功")
                : error ||
                  result?.error ||
                  text(
                    locale,
                    "暂无可识别的用量数据",
                    "No recognized usage data",
                    "認識できる使用量データがありません",
                  )}
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            <button className="btn btn-secondary btn-sm" type="button" onClick={() => void load()} disabled={loading}>
              <RefreshCw size={13} className={loading ? "spin" : undefined} />
              {text(locale, "重试", "Retry", "再試行")}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              type="button"
              onClick={() => void copyResult()}
              disabled={!result}
            >
              <Copy size={13} />
              {text(locale, "复制", "Copy", "コピー")}
            </button>
          </div>
        </div>

        {rows.length > 0 ? (
          <div className="section-card" style={{ marginTop: 14, padding: 12 }}>
            {rows.map((row, rowIndex) => (
              <div
                key={rowIndex}
                style={{
                  display: "grid",
                  gridTemplateColumns: "minmax(120px, 0.7fr) minmax(0, 1.3fr)",
                  gap: 8,
                  fontSize: 12,
                  padding: "6px 0",
                  borderBottom: rowIndex === rows.length - 1 ? "none" : "1px solid var(--border-subtle)",
                }}
              >
                <>
                  {Object.entries(row as Record<string, unknown>).map(([key, value]) => (
                    <div key={`${rowIndex}-${key}`} style={{ display: "contents" }}>
                      <span style={{ color: "var(--text-muted)" }}>{key}</span>
                      <span style={{ overflowWrap: "anywhere" }}>{formatValue(value)}</span>
                    </div>
                  ))}
                </>
              </div>
            ))}
          </div>
        ) : null}

        <details style={{ marginTop: 14 }} open={rows.length === 0}>
          <summary style={{ cursor: "pointer", color: "var(--text-secondary)", fontSize: 12 }}>
            {text(locale, "查看原始标准化结果", "View normalized result", "正規化された結果を表示")}
          </summary>
          <pre
            style={{
              marginTop: 8,
              padding: 12,
              borderRadius: 8,
              background: "var(--bg-input)",
              color: "var(--text-secondary)",
              fontSize: 11,
              lineHeight: 1.5,
              whiteSpace: "pre-wrap",
              overflowWrap: "anywhere",
            }}
          >
            {formatJson(result)}
          </pre>
        </details>
      </div>
    </div>
  );
}
