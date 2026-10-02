import { Suspense, useMemo, useState } from "react";
import { Copy, Gauge, RefreshCw } from "lucide-react";
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
import LoadingState from "./states/LoadingState";
import { useUsageResult } from "./UsageDetailsDialog/useUsageResult";
import { UsageCards } from "./UsageDetailsDialog/UsageCards";
import { text, usageRows } from "./UsageDetailsDialog/presentation";
import UsageAlertRule from "./usageAlerts/UsageAlertRule";

import CodeEditor from "./DeferredCodeEditor";

interface UsageDetailsDialogProps {
  profile: ConfigProfile | null;
  locale: string;
  onClose: () => void;
}

export default function UsageDetailsDialog({ profile, locale, onClose }: UsageDetailsDialogProps) {
  const { result, loading, error, updatedAt, refresh } = useUsageResult(profile);
  const [rawOpen, setRawOpen] = useState(false);
  const rows = useMemo(() => (result?.success ? usageRows(result.data) : []), [result]);
  const json = useMemo(() => JSON.stringify(result ?? {}, null, 2), [result]);
  const failure = error || result?.error;
  const cached = Boolean(
    result?.stale || result?.asOf != null || rows.some((row) => row.stale === true || row.asOf != null),
  );
  const stale = Boolean(error && result?.success);
  if (!profile) return null;

  const copyResult = async () => {
    try {
      await navigator.clipboard.writeText(json);
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
      <DialogContent className="max-w-[720px] max-h-[min(680px,calc(100dvh-40px))]">
        <DialogHeader>
          <div className="grid size-8 shrink-0 place-items-center rounded-[6px] bg-secondary text-muted-foreground">
            <Gauge size={16} aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <DialogTitle>{text(locale, "配置用量与余额", "Usage and balance", "使用量と残高")}</DialogTitle>
            <DialogDescription className="break-words">
              {profile.name} · {profile.tool_id}
            </DialogDescription>
          </div>
        </DialogHeader>
        <DialogBody className="space-y-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 space-y-1 text-xs" role="status" aria-live="polite">
              <p
                className={
                  failure || cached
                    ? "text-[var(--warning)]"
                    : result?.success
                      ? "text-[var(--success)]"
                      : "text-muted-foreground"
                }
              >
                {loading
                  ? rows.length
                    ? text(
                        locale,
                        "正在刷新，显示上次查询结果…",
                        "Refreshing; showing the previous result…",
                        "更新中です。前回の結果を表示しています…",
                      )
                    : text(locale, "正在查询…", "Querying…", "照会中…")
                  : cached
                    ? text(
                        locale,
                        "供应商返回了缓存数据，请留意数据时间",
                        "The result includes provider-cached data; check the data timestamp",
                        "キャッシュデータが含まれます。データの日時を確認してください",
                      )
                    : stale
                      ? text(
                          locale,
                          "刷新失败，下方为上次成功查询的数据",
                          "Refresh failed; showing the last successful result",
                          "更新に失敗しました。前回成功した結果を表示しています",
                        )
                      : failure
                        ? text(locale, "查询失败", "Query failed", "照会に失敗しました")
                        : rows.length
                          ? text(locale, "查询成功", "Query succeeded", "照会成功")
                          : text(
                              locale,
                              "暂无可识别的用量数据",
                              "No recognized usage data",
                              "認識できる使用量データがありません",
                            )}
              </p>
              {updatedAt && (
                <p className="text-muted-foreground">
                  {text(locale, "更新于", "Updated", "更新日時")}{" "}
                  {updatedAt.toLocaleTimeString(locale === "zh" ? "zh-CN" : locale === "ja" ? "ja-JP" : "en-US")}
                </p>
              )}
            </div>
            <div className="flex shrink-0 gap-2">
              <Button variant="secondary" type="button" onClick={() => void refresh()} disabled={loading}>
                <RefreshCw size={14} className={loading ? "spin" : undefined} aria-hidden="true" />
                {text(locale, "刷新", "Refresh", "更新")}
              </Button>
              <Button variant="secondary" type="button" onClick={() => void copyResult()} disabled={!result}>
                <Copy size={14} aria-hidden="true" />
                {text(locale, "复制", "Copy", "コピー")}
              </Button>
            </div>
          </div>
          {failure && (
            <p
              role="alert"
              className="rounded-md border border-border bg-secondary p-3 text-xs leading-relaxed break-words text-muted-foreground"
            >
              {failure}
            </p>
          )}
          {loading && !result && (
            <LoadingState
              label={text(locale, "读取余额与额度…", "Reading balance and quota…", "残高と割当を読み込み中…")}
            />
          )}
          {rows.length > 0 && <UsageCards rows={rows} locale={locale} />}
          <UsageAlertRule
            key={JSON.stringify([profile.id, profile.tool_id, profile.config_snapshot])}
            profileId={profile.id}
            locale={locale}
          />
          {!loading && !failure && rows.length === 0 && (
            <p className="rounded-md border border-border p-4 text-xs leading-relaxed text-muted-foreground">
              {text(
                locale,
                "供应商未返回可展示的用量。可在配置中设置用量查询脚本。",
                "The provider returned no displayable usage. Configure a usage query script in this profile.",
                "表示できる使用量が返されませんでした。設定で使用量照会スクリプトを指定できます。",
              )}
            </p>
          )}
          {result && (
            <details className="min-w-0" onToggle={(event) => setRawOpen(event.currentTarget.open)}>
              <summary className="cursor-pointer text-xs text-muted-foreground hover:text-foreground">
                {text(locale, "查看标准化 JSON", "View normalized JSON", "正規化 JSON を表示")}
              </summary>
              {rawOpen && (
                <div className="mt-3 min-w-0">
                  <Suspense fallback={<LoadingState />}>
                    <CodeEditor value={json} language="json" readOnly minHeight={160} maxHeight={300} />
                  </Suspense>
                </div>
              )}
            </details>
          )}
        </DialogBody>
        <DialogFooter>
          <Button variant="secondary" onClick={onClose}>
            {text(locale, "关闭", "Close", "閉じる")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
