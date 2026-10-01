import type { CopilotQuota } from "../../lib/copilotAccounts";

export function QuotaItem({
  label,
  value,
  unknownLabel,
  unlimitedLabel,
}: {
  label: string;
  value: CopilotQuota | null;
  unknownLabel: string;
  unlimitedLabel: string;
}) {
  const remaining = value?.remaining;
  const entitlement = value?.entitlement;
  const reported = value?.percent_remaining;
  const knownCount =
    typeof remaining === "number" &&
    Number.isFinite(remaining) &&
    typeof entitlement === "number" &&
    Number.isFinite(entitlement) &&
    entitlement >= 0 &&
    remaining >= 0;
  const percent = value?.unlimited
    ? 100
    : typeof reported === "number" && Number.isFinite(reported)
      ? Math.max(0, Math.min(100, reported))
      : knownCount && entitlement > 0
        ? Math.max(0, Math.min(100, (100 * remaining) / entitlement))
        : null;
  const count = (number: number) => number.toLocaleString(undefined, { maximumFractionDigits: 2 });
  const display = value?.unlimited
    ? unlimitedLabel
    : knownCount
      ? `${count(remaining)} / ${count(entitlement)}`
      : unknownLabel;
  return (
    <div className="grid min-w-0 gap-2">
      <div className="flex items-start justify-between gap-3 text-[12px]">
        <span>{label}</span>
        <span className="text-right tabular-nums text-muted-foreground">{display}</span>
      </div>
      {percent !== null ? (
        <div
          role="progressbar"
          aria-label={label}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
          aria-valuetext={display}
          className="h-1.5 overflow-hidden rounded-sm bg-[var(--bg-card-hover)]"
        >
          <div
            className="h-full rounded-sm"
            style={{ width: `${percent}%`, background: percent < 20 ? "var(--warning)" : "var(--accent)" }}
          />
        </div>
      ) : (
        <div className="h-1.5 rounded-sm bg-[var(--bg-card-hover)]" aria-hidden="true" />
      )}
    </div>
  );
}
