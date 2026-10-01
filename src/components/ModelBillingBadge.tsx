import { normalizeModelBilling } from "../lib/modelBilling";
import { getLocale } from "../lib/i18n";

export type BillingText = (zh: string, en: string, ja?: string) => string;

export function modelBillingLabel(value: unknown, text: BillingText) {
  const billing = normalizeModelBilling(value);
  if (billing.kind === "free") return text("不消耗高级额度", "No premium quota", "Premium クォータ消費なし");
  if (billing.kind === "premium")
    return billing.multiplier === null
      ? text("高级请求 · 倍率未确定", "Premium · Multiplier uncertain", "Premium · 倍率未確定")
      : text(`高级请求 ×${billing.multiplier}`, `Premium ×${billing.multiplier}`, `Premium ×${billing.multiplier}`);
  return text("计费信息未提供", "Billing not reported", "課金情報未報告");
}

export default function ModelBillingBadge({ value, localeText }: { value: unknown; localeText?: BillingText }) {
  const locale = getLocale();
  const text: BillingText = localeText ?? ((zh, en, ja) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en));
  return (
    <span className="badge badge-muted max-w-full break-words text-[11px] font-medium">
      {modelBillingLabel(value, text)}
    </span>
  );
}
