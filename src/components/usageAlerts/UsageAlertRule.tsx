import { useId, useState } from "react";
import { Bell, ChevronDown, Plus, Trash2 } from "lucide-react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Switch } from "../ui/switch";
import { text } from "../UsageDetailsDialog/presentation";
import { useUsageAlerts } from "./useUsageAlerts";
import { defaultSettings, validateSettings, type AlertRule, type AlertSettings } from "./types";

interface Props {
  profileId: string;
  locale: string;
}

export default function UsageAlertRule({ profileId, locale }: Props) {
  const [open, setOpen] = useState(false);
  const panelId = useId();
  const { data, error, busy, action, refresh } = useUsageAlerts();
  const rule = data?.rules.find((item) => item.profileId === profileId);
  const tx = (zh: string, en: string, ja: string) => text(locale, zh, en, ja);
  return (
    <section className="min-w-0 rounded-lg border border-border">
      <Button
        variant="ghost"
        className="w-full justify-start gap-2"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen(!open)}
      >
        <Bell size={14} aria-hidden="true" />
        {tx("余额与额度提醒", "Balance and quota alerts", "残高・割当の通知")}
        <span className="ml-auto text-[11px] text-muted-foreground">
          {rule?.paused
            ? tx("已暂停", "Paused", "一時停止")
            : rule?.settings.enabled
              ? tx("已开启", "On", "有効")
              : tx("未开启", "Off", "無効")}
        </span>
        <ChevronDown size={14} className={open ? "rotate-180" : undefined} aria-hidden="true" />
      </Button>
      {open && (
        <div id={panelId} className="space-y-3 border-t border-border p-3">
          {error && (
            <div role="alert" className="space-y-2 text-xs text-[var(--warning)]">
              <p className="break-words">{error}</p>
              <Button variant="secondary" onClick={() => void refresh()}>
                {tx("重试加载", "Retry loading", "再読み込み")}
              </Button>
            </div>
          )}
          {!data && !error && (
            <p role="status" className="text-xs text-muted-foreground">
              {tx("正在读取提醒设置…", "Loading alert settings…", "通知設定を読み込み中…")}
            </p>
          )}
          {data && (
            <RuleForm
              profileId={profileId}
              locale={locale}
              rule={rule}
              busy={busy}
              save={(settings) =>
                action("set_usage_alert_rule", { profileId, settings, expectedIdentity: rule?.queryIdentity ?? "" })
              }
            />
          )}
        </div>
      )}
    </section>
  );
}

function RuleForm({
  profileId,
  locale,
  rule,
  busy,
  save,
}: Props & { rule?: AlertRule; busy: boolean; save: (settings: AlertSettings) => Promise<boolean> }) {
  const [settings, setSettings] = useState<AlertSettings>(() => rule?.settings ?? defaultSettings);
  const [saved, setSaved] = useState(false);
  const id = useId();
  const tx = (zh: string, en: string, ja: string) => text(locale, zh, en, ja);
  const update = (patch: Partial<AlertSettings>) => {
    setSettings((previous) => ({ ...previous, ...patch }));
    setSaved(false);
  };
  const balance = (index: number, patch: Partial<AlertSettings["balances"][number]>) =>
    update({
      balances: settings.balances.map((item, candidate) => (candidate === index ? { ...item, ...patch } : item)),
    });
  const valid = validateSettings(settings);
  return (
    <form
      className="space-y-3"
      onSubmit={(event) => {
        event.preventDefault();
        if (valid && !busy) void save(settings).then((success) => setSaved(success));
      }}
      aria-label={tx("提醒设置", "Alert settings", "通知設定")}
    >
      <div className="flex min-h-8 items-center justify-between gap-3">
        <label htmlFor={`${id}-enabled`} className="text-sm">
          {tx("自动检查并提醒", "Automatic checks and alerts", "自動確認と通知")}
        </label>
        <Switch
          id={`${id}-enabled`}
          checked={settings.enabled}
          disabled={busy}
          onCheckedChange={(enabled) =>
            update({
              enabled,
              ...(enabled && settings.quotaPercent === null && settings.balances.length === 0
                ? { quotaPercent: 80 }
                : {}),
            })
          }
        />
      </div>
      <p className="text-xs leading-relaxed text-muted-foreground">
        {tx(
          "仅在软件运行时每 5 分钟查询。启用后会执行此配置的用量脚本或请求供应商接口。",
          "Checks every 5 minutes while the app runs, using this profile’s usage script or provider API.",
          "アプリ実行中に5分ごとに、この設定のスクリプトまたはAPIで確認します。",
        )}
      </p>
      {rule?.paused && (
        <p role="status" className="text-xs text-[var(--warning)]">
          {tx(
            "账号或查询配置已变更，后台检查已暂停。保存后绑定当前配置。",
            "Account or query settings changed. Monitoring is paused; save to bind the current configuration.",
            "アカウントまたは照会設定が変わりました。保存すると現在の設定に紐づけます。",
          )}
        </p>
      )}
      {!rule?.queryIdentity && (
        <p role="status" className="text-xs text-[var(--warning)]">
          {tx(
            "当前配置无法用于用量提醒，请检查工具类型及配置内容。",
            "This configuration cannot be used for usage alerts. Check its tool type and configuration contents.",
            "この設定では使用量通知を利用できません。ツールの種類と設定内容を確認してください。",
          )}
        </p>
      )}
      {rule?.status === "query_failed" && (
        <p role="status" className="text-xs text-[var(--warning)]">
          {tx(
            "上次后台查询失败，未触发新提醒；已保存的历史仍保留。",
            "The last background query failed; no new alerts were triggered. Saved history is retained.",
            "前回の照会に失敗したため新しい通知はありません。保存済みの履歴は保持されます。",
          )}
        </p>
      )}
      {rule?.checkedAt && (
        <p className="text-[11px] text-muted-foreground">
          {tx("上次检查", "Last checked", "前回の確認")}:{" "}
          {new Date(rule.checkedAt * 1000).toLocaleString(
            locale === "zh" ? "zh-CN" : locale === "ja" ? "ja-JP" : "en-US",
          )}
        </p>
      )}
      <div className="flex min-h-8 flex-wrap items-center gap-3">
        <Switch
          id={`${id}-quota`}
          checked={settings.quotaPercent !== null}
          disabled={busy}
          onCheckedChange={(enabled) => update({ quotaPercent: enabled ? 80 : null })}
        />
        <label htmlFor={`${id}-quota`} className="text-xs">
          {tx("额度使用达到", "Quota usage reaches", "割当使用率")}
        </label>
        {settings.quotaPercent !== null && (
          <div className="grid grid-cols-[80px_auto] items-center gap-2">
            <Input
              aria-label={tx("额度百分比", "Quota percentage", "割当の割合")}
              type="number"
              min={1}
              max={100}
              step="any"
              disabled={busy}
              value={Number.isFinite(settings.quotaPercent) ? settings.quotaPercent : ""}
              onChange={(event) =>
                update({ quotaPercent: event.target.value === "" ? NaN : Number(event.target.value) })
              }
            />
            <span className="text-xs">%</span>
          </div>
        )}
      </div>
      <div className="space-y-2">
        <div className="flex min-h-8 items-center justify-between gap-2">
          <span className="text-xs">{tx("余额不高于", "Balance at or below", "残高が以下")}</span>
          <Button
            type="button"
            variant="ghost"
            disabled={busy || settings.balances.length >= 10}
            onClick={() => update({ balances: [...settings.balances, { unit: "", amount: 5 }] })}
          >
            <Plus size={14} />
            {tx("添加阈值", "Add threshold", "しきい値を追加")}
          </Button>
        </div>
        {settings.balances.map((item, index) => (
          <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_96px_32px] gap-2" key={index}>
            <Input
              aria-label={`${tx("余额单位", "Balance unit", "残高単位")} ${index + 1}`}
              placeholder="USD / CNY / credits"
              className="min-w-0"
              maxLength={32}
              disabled={busy}
              value={item.unit}
              onChange={(event) => balance(index, { unit: event.target.value })}
            />
            <Input
              aria-label={`${tx("余额阈值", "Balance threshold", "残高しきい値")} ${index + 1}`}
              type="number"
              min={0}
              step="any"
              disabled={busy}
              value={Number.isFinite(item.amount) ? item.amount : ""}
              onChange={(event) =>
                balance(index, { amount: event.target.value === "" ? NaN : Number(event.target.value) })
              }
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              disabled={busy}
              aria-label={`${tx("移除阈值", "Remove threshold", "しきい値を削除")} ${index + 1}`}
              onClick={() => update({ balances: settings.balances.filter((_, candidate) => candidate !== index) })}
            >
              <Trash2 size={14} />
            </Button>
          </div>
        ))}
        {settings.balances.length > 0 && (
          <p className="text-[11px] leading-relaxed text-muted-foreground">
            {tx(
              "单位须与查询结果一致；不同货币和积分分别设置。",
              "Match the query’s unit exactly; configure each currency or credit unit separately.",
              "照会結果と同じ単位を指定し、通貨やクレジットごとに設定してください。",
            )}
          </p>
        )}
      </div>
      <div className="flex min-h-8 items-center justify-between gap-3">
        <label htmlFor={`${id}-system`} className="text-xs">
          {tx("同时发送系统通知", "Also send system notifications", "システム通知も送信")}
        </label>
        <Switch
          id={`${id}-system`}
          checked={settings.systemNotifications}
          disabled={busy}
          onCheckedChange={(systemNotifications) => update({ systemNotifications })}
        />
      </div>
      <p className="text-[11px] leading-relaxed text-muted-foreground">
        {tx(
          "提醒保存在顶部通知中心。同一额度窗口只提醒一次；系统通知能否显示取决于系统设置。",
          "Alerts remain in the notification center. Each known quota window alerts once; visibility of system notifications depends on OS settings.",
          "通知は通知センターに保存されます。同じ割当期間の通知は一度です。システム通知の表示はOS設定に依存します。",
        )}
      </p>
      {!valid && (
        <p role="alert" className="text-xs text-[var(--warning)]">
          {tx(
            "请设置 1–100% 的额度阈值，或填写唯一单位及非负余额阈值。",
            "Set a 1–100% quota threshold or unique balance units with nonnegative thresholds.",
            "1–100%の割当、または重複しない単位と0以上の残高を設定してください。",
          )}
        </p>
      )}
      <div className="flex flex-wrap items-center justify-end gap-3">
        <span role="status" className="text-xs text-muted-foreground">
          {saved ? tx("已保存", "Saved", "保存済み") : ""}
        </span>
        <Button type="submit" disabled={busy || !valid || !profileId || !rule?.queryIdentity}>
          {busy ? tx("保存中…", "Saving…", "保存中…") : tx("保存提醒设置", "Save alert settings", "通知設定を保存")}
        </Button>
      </div>
    </form>
  );
}
