export function text(locale: string, zh: string, en: string, ja: string) {
  return locale === "zh" ? zh : locale === "ja" ? ja : en;
}

export function usageRows(data: unknown): Record<string, unknown>[] {
  const items = Array.isArray(data) ? data : [data];
  return items.filter(
    (item): item is Record<string, unknown> => Boolean(item) && typeof item === "object" && !Array.isArray(item),
  );
}

export function number(value: unknown): number | null {
  if (typeof value !== "number" && (typeof value !== "string" || !value.trim())) return null;
  const result = Number(value);
  return Number.isFinite(result) ? result : null;
}

export function usagePercent(row: Record<string, unknown>): number | null {
  const explicit = number(row.utilization ?? row.percentage);
  if (explicit !== null && explicit >= 0) return Math.min(explicit, 100);
  const total = number(row.total ?? row.limit);
  if (total === null || total <= 0) return null;
  const used = number(row.used);
  const remaining = number(row.remaining);
  const ratio = used !== null && used >= 0 ? used / total : remaining !== null ? 1 - remaining / total : null;
  const percent = ratio === null ? null : ratio * 100;
  return percent !== null && Number.isFinite(percent) ? Math.max(0, Math.min(percent, 100)) : null;
}

export function formatNumber(value: number, locale: string) {
  return new Intl.NumberFormat(locale === "zh" ? "zh-CN" : locale === "ja" ? "ja-JP" : "en-US", {
    maximumSignificantDigits: 7,
  }).format(value);
}

export function resetTime(value: unknown, locale: string): string | null {
  // Require an explicit timestamp, never interpret arbitrary strings as dates.
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(value)) return null;
  const date = new Date(value);
  return Number.isFinite(date.getTime())
    ? date.toLocaleString(locale === "zh" ? "zh-CN" : locale === "ja" ? "ja-JP" : "en-US")
    : null;
}

export function windowName(value: unknown, locale: string, index: number) {
  if (value === "five_hour") return text(locale, "5 小时额度", "5-hour quota", "5 時間の割当");
  if (value === "weekly_limit") return text(locale, "每周额度", "Weekly quota", "週間の割当");
  return typeof value === "string" && value.trim()
    ? value
    : text(locale, `额度 ${index + 1}`, `Quota ${index + 1}`, `割当 ${index + 1}`);
}
