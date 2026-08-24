import { useCallback, useEffect, useMemo, useState } from "react";
import { Copy, Gauge, RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "./Toast";
import type { ConfigProfile } from "../pages/profiles/helpers";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

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
  }, [load, profile]);

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
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-[720px]">
        <DialogHeader>
          <div className="grid size-9 shrink-0 place-items-center rounded-[7px] bg-primary/10 text-primary">
            <Gauge size={17} aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <DialogTitle>{text(locale, "Provider 用量", "Provider usage", "Provider 使用量")}</DialogTitle>
            <DialogDescription className="truncate">
              {profile.name} · {profile.tool_id}
            </DialogDescription>
          </div>
        </DialogHeader>
        <DialogBody>
          <div className="flex flex-wrap items-center justify-between gap-2">
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
            <div className="flex gap-2">
              <Button variant="secondary" size="sm" type="button" onClick={() => void load()} disabled={loading}>
                <RefreshCw size={13} className={loading ? "spin" : undefined} />
                {text(locale, "重试", "Retry", "再試行")}
              </Button>
              <Button variant="secondary" size="sm" type="button" onClick={() => void copyResult()} disabled={!result}>
                <Copy size={13} />
                {text(locale, "复制", "Copy", "コピー")}
              </Button>
            </div>
          </div>

          {rows.length > 0 ? (
            <div className="mt-3 rounded-[6px] border border-border bg-[var(--bg-elevated)]/45 p-3">
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
                  {Object.entries(row as Record<string, unknown>).map(([key, value]) => (
                    <div key={`${rowIndex}-${key}`} style={{ display: "contents" }}>
                      <span style={{ color: "var(--text-muted)" }}>{key}</span>
                      <span style={{ overflowWrap: "anywhere" }}>{formatValue(value)}</span>
                    </div>
                  ))}
                </div>
              ))}
            </div>
          ) : null}

          <details className="mt-3" open={rows.length === 0}>
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
        </DialogBody>
        <DialogFooter>
          <Button variant="secondary" size="sm" onClick={onClose}>
            {text(locale, "关闭", "Close", "閉じる")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
