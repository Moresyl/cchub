import { useState } from "react";
import { Bell, CheckCheck, RefreshCw } from "lucide-react";
import { Button } from "../ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";
import { formatNumber, resetTime, text, windowName } from "../UsageDetailsDialog/presentation";
import { useUsageAlerts } from "./useUsageAlerts";

export default function NotificationCenter({ locale }: { locale: string }) {
  const [open, setOpen] = useState(false);
  const { data, error, busy, action, refresh } = useUsageAlerts();
  const tx = (zh: string, en: string, ja: string) => text(locale, zh, en, ja);
  const events = data?.events ?? [];
  const unread = events.filter((event) => !event.read).length;
  const checking = busy || data?.polling;
  const failed = data?.rules.filter((rule) => rule.settings.enabled && rule.status === "query_failed").length ?? 0;
  const paused = data?.rules.filter((rule) => rule.paused).length ?? 0;
  const statuses: Record<string, string> = {
    accepted: tx("已交给系统", "Submitted to system", "システムに送信済み"),
    pending: tx("等待系统通知", "System notification pending", "システム通知待ち"),
    failed: tx("系统通知失败", "System notification failed", "システム通知に失敗"),
    cancelled: tx("系统通知已取消", "System notification cancelled", "システム通知を取消済み"),
    off: tx("应用内提醒", "In-app alert", "アプリ内通知"),
  };
  return (
    <>
      <Button
        variant="ghost"
        size="icon"
        className="relative"
        aria-label={`${tx("通知中心", "Notification center", "通知センター")}${unread ? ` · ${unread}` : ""}`}
        title={tx("通知中心", "Notification center", "通知センター")}
        onClick={() => {
          setOpen(true);
          void refresh();
        }}
      >
        <Bell size={15} aria-hidden="true" />
        {unread > 0 && (
          <span aria-hidden="true" className="absolute right-1 top-1 size-1.5 rounded-full bg-[var(--warning)]" />
        )}
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-[620px] max-h-[min(680px,calc(100dvh-40px))]">
          <DialogHeader>
            <div className="min-w-0">
              <DialogTitle>{tx("通知中心", "Notification center", "通知センター")}</DialogTitle>
              <DialogDescription>
                {tx("余额与额度提醒历史", "Balance and quota alert history", "残高と割当の通知履歴")}
              </DialogDescription>
            </div>
          </DialogHeader>
          <DialogBody className="space-y-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="text-xs text-muted-foreground">
                {tx(`${unread} 条未读`, `${unread} unread`, `未読 ${unread} 件`)}
              </span>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="secondary"
                  disabled={checking || !data?.rules.some((rule) => rule.settings.enabled && !rule.paused)}
                  onClick={() => void action("check_usage_alerts")}
                >
                  <RefreshCw size={14} className={checking ? "spin" : undefined} />
                  {checking ? tx("检查中…", "Checking…", "確認中…") : tx("立即检查", "Check now", "今すぐ確認")}
                </Button>
                <Button
                  variant="secondary"
                  disabled={busy || !unread}
                  onClick={() => void action("mark_usage_alerts_read")}
                >
                  <CheckCheck size={14} />
                  {tx("全部已读", "Mark all read", "すべて既読")}
                </Button>
              </div>
            </div>
            {(failed > 0 || paused > 0) && (
              <p
                role="status"
                className="rounded-md border border-border p-3 text-xs leading-relaxed text-[var(--warning)]"
              >
                {tx(
                  `${failed} 个配置上次查询失败，${paused} 个配置因账号变更暂停。请在对应配置的提醒设置中查看。`,
                  `${failed} profiles failed their last check; ${paused} are paused after account changes. Review their alert settings.`,
                  `前回の照会に失敗した設定は${failed}件、アカウント変更で停止中は${paused}件です。各設定の通知設定を確認してください。`,
                )}
              </p>
            )}
            {error && (
              <div role="alert" className="space-y-2 text-xs">
                <p className="break-words text-[var(--warning)]">{error}</p>
                <Button variant="secondary" onClick={() => void refresh()}>
                  {tx("重新加载", "Reload", "再読み込み")}
                </Button>
              </div>
            )}
            {!data && !error && (
              <p role="status" className="text-xs text-muted-foreground">
                {tx("正在加载…", "Loading…", "読み込み中…")}
              </p>
            )}
            {data && !events.length && (
              <div className="rounded-lg border border-border p-5 text-center">
                <Bell size={20} className="mx-auto mb-3 text-muted-foreground" />
                <p className="text-sm">{tx("暂无提醒", "No alerts yet", "通知はありません")}</p>
                <p className="mt-2 text-xs leading-relaxed text-muted-foreground">
                  {tx(
                    "在配置的用量与余额窗口中开启提醒。历史仅保存在这台设备上。",
                    "Enable alerts in a profile’s usage and balance window. History stays on this device.",
                    "設定の使用量・残高画面で通知を有効にできます。履歴はこの端末に保存されます。",
                  )}
                </p>
              </div>
            )}
            {events.map((event) => (
              <article key={event.id} className="min-w-0 space-y-2 rounded-lg border border-border bg-secondary/30 p-3">
                <div className="flex items-start gap-2">
                  <span
                    className={`mt-1.5 size-1.5 shrink-0 rounded-full ${event.read ? "bg-muted-foreground/40" : "bg-[var(--warning)]"}`}
                    aria-label={event.read ? tx("已读", "Read", "既読") : tx("未读", "Unread", "未読")}
                  />
                  <div className="min-w-0 flex-1">
                    <p className="break-words text-sm font-medium">{event.profileName}</p>
                    <p className="mt-1 break-words text-xs text-muted-foreground">
                      {windowName(event.label, locale, 0)} · {event.toolId}
                    </p>
                  </div>
                  <time
                    className="shrink-0 text-[11px] text-muted-foreground"
                    dateTime={new Date(event.createdAt * 1000).toISOString()}
                  >
                    {new Date(event.createdAt * 1000).toLocaleDateString(
                      locale === "zh" ? "zh-CN" : locale === "ja" ? "ja-JP" : "en-US",
                    )}
                  </time>
                </div>
                <p className="break-words text-xs">
                  {event.kind === "quota"
                    ? tx(
                        `使用率 ${formatNumber(event.value, locale)}%，达到 ${formatNumber(event.threshold, locale)}% 提醒阈值`,
                        `Usage ${formatNumber(event.value, locale)}%, reaching the ${formatNumber(event.threshold, locale)}% alert threshold`,
                        `使用率 ${formatNumber(event.value, locale)}%、通知しきい値 ${formatNumber(event.threshold, locale)}%`,
                      )
                    : tx(
                        `余额 ${formatNumber(event.value, locale)} ${event.unit ?? ""}，不高于 ${formatNumber(event.threshold, locale)}`,
                        `Balance ${formatNumber(event.value, locale)} ${event.unit ?? ""}, at or below ${formatNumber(event.threshold, locale)}`,
                        `残高 ${formatNumber(event.value, locale)} ${event.unit ?? ""}、しきい値 ${formatNumber(event.threshold, locale)} 以下`,
                      )}
                </p>
                {event.resetAt && (
                  <p className="text-[11px] text-muted-foreground">
                    {tx("额度重置", "Quota resets", "割当リセット")}:{" "}
                    {resetTime(new Date(event.resetAt * 1000).toISOString(), locale)}
                  </p>
                )}
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <span className="text-[11px] text-muted-foreground">
                    {statuses[event.systemStatus] ?? event.systemStatus}
                  </span>
                  <div className="flex gap-2">
                    {event.systemStatus === "failed" && (
                      <Button
                        variant="ghost"
                        disabled={busy}
                        onClick={() => void action("retry_usage_alert", { eventId: event.id })}
                      >
                        {tx("重试系统通知", "Retry system notification", "システム通知を再試行")}
                      </Button>
                    )}
                    {!event.read && (
                      <Button
                        variant="ghost"
                        disabled={busy}
                        onClick={() => void action("mark_usage_alerts_read", { eventId: event.id })}
                      >
                        {tx("标为已读", "Mark read", "既読にする")}
                      </Button>
                    )}
                  </div>
                </div>
              </article>
            ))}
          </DialogBody>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setOpen(false)}>
              {tx("关闭", "Close", "閉じる")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
