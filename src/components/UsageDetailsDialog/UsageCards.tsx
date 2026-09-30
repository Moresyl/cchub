import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { formatNumber, number, resetTime, text, usagePercent, windowName } from "./presentation";

export function UsageCards({ rows, locale }: { rows: Record<string, unknown>[]; locale: string }) {
  return (
    <div className="grid min-w-0 gap-3 sm:grid-cols-2">
      {rows.map((row, index) => {
        const remaining = number(row.remaining);
        const used = number(row.used);
        const total = number(row.total ?? row.limit);
        const percentage = usagePercent(row);
        const reset = resetTime(row.resetAt ?? row.resetsAt, locale);
        const unit = typeof row.unit === "string" ? row.unit : "";
        const metric =
          row.metric === "tokens_limit"
            ? "Token"
            : row.metric === "credit_limit"
              ? text(locale, "积分", "Credits", "クレジット")
              : null;
        const title = `${windowName(row.planName ?? row.name, locale, index)}${metric ? ` · ${metric}` : ""}`;
        return (
          <Card key={index} className="min-w-0 shadow-none">
            <CardHeader className="gap-3">
              <CardTitle className="break-words leading-5">{title}</CardTitle>
              <div>
                <div className="text-xs text-muted-foreground">
                  {remaining !== null
                    ? text(locale, "剩余额度 / 余额", "Remaining quota / balance", "残りの割当 / 残高")
                    : text(locale, "已用比例", "Used", "使用率")}
                </div>
                <div className="mt-1 flex flex-wrap items-baseline gap-2 tabular-nums">
                  <span className="text-2xl font-semibold">
                    {remaining !== null
                      ? formatNumber(remaining, locale)
                      : percentage !== null
                        ? `${formatNumber(percentage, locale)}%`
                        : "—"}
                  </span>
                  {remaining !== null && unit && (
                    <span className="text-xs text-muted-foreground break-all">{unit}</span>
                  )}
                </div>
              </div>
            </CardHeader>
            <CardContent className="space-y-3 text-xs">
              {percentage !== null && (
                <div>
                  <div className="mb-1.5 flex justify-between gap-2 text-muted-foreground">
                    <span>{text(locale, "额度使用", "Quota used", "割当の使用率")}</span>
                    <span>{formatNumber(percentage, locale)}%</span>
                  </div>
                  <div
                    role="progressbar"
                    aria-label={title}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={percentage}
                    className="h-1.5 overflow-hidden rounded-sm bg-secondary"
                  >
                    <div
                      className="h-full rounded-sm"
                      style={{
                        width: `${percentage}%`,
                        background:
                          percentage >= 90
                            ? "var(--danger)"
                            : percentage >= 75
                              ? "var(--warning)"
                              : "var(--text-secondary)",
                      }}
                    />
                  </div>
                </div>
              )}
              {(used !== null || total !== null) && (
                <dl className="grid grid-cols-2 gap-3">
                  {used !== null && (
                    <div>
                      <dt className="text-muted-foreground">{text(locale, "已用", "Used", "使用済み")}</dt>
                      <dd className="mt-1 break-all tabular-nums">
                        {formatNumber(used, locale)} {unit}
                      </dd>
                    </div>
                  )}
                  {total !== null && (
                    <div>
                      <dt className="text-muted-foreground">{text(locale, "总额度", "Total quota", "割当合計")}</dt>
                      <dd className="mt-1 break-all tabular-nums">
                        {formatNumber(total, locale)} {unit}
                      </dd>
                    </div>
                  )}
                </dl>
              )}
              {reset && (
                <p className="text-muted-foreground break-words">
                  {text(locale, "重置时间", "Resets", "リセット日時")} · {reset}
                </p>
              )}
              {row.isValid === false && (
                <p className="text-[var(--warning)]">
                  {text(
                    locale,
                    "供应商报告此额度当前不可用",
                    "Provider reports this quota as unavailable",
                    "この割当は現在利用できません",
                  )}
                </p>
              )}
              {remaining === null && percentage === null && (
                <p className="text-muted-foreground">
                  {text(
                    locale,
                    "数据不完整，无法计算剩余或使用比例",
                    "Incomplete data; remaining quota and percentage are unknown",
                    "データが不完全なため、残りの割当と使用率は不明です",
                  )}
                </p>
              )}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
