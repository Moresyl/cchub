import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown, RefreshCw } from "lucide-react";
import { t } from "../../lib/i18n";
import { Button } from "../../components/ui/button";
import NumberRow from "./NumberRow";
import type { AdmissionConfig } from "./types";

interface Entry {
  key: string;
  profiles: string[];
  active: number;
  queued: number;
  limit: number;
}
interface Stats {
  entries: Entry[];
  active: number;
  queued: number;
}
export const DEFAULT_ADMISSION: AdmissionConfig = {
  maxConcurrent: 0,
  maxQueued: 32,
  queueTimeoutSecs: 30,
  accountLimits: {},
  accountLabels: {},
};

export default function AdmissionPanel({
  config = DEFAULT_ADMISSION,
  onChange,
}: {
  config?: AdmissionConfig;
  onChange: (config: AdmissionConfig) => void;
}) {
  const i = t().proxyAdvanced;
  const [expanded, setExpanded] = useState(false);
  const [stats, setStats] = useState<Stats | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(false);
  const generation = useRef(0);
  useEffect(
    () => () => {
      generation.current++;
    },
    [],
  );

  async function refresh() {
    const id = ++generation.current;
    setLoading(true);
    try {
      const next = await invoke<Stats>("get_proxy_admission_stats");
      if (generation.current === id) {
        setStats(next);
        setError(false);
      }
    } catch {
      if (generation.current === id) setError(true);
    } finally {
      if (generation.current === id) setLoading(false);
    }
  }
  function patch(value: Partial<AdmissionConfig>) {
    onChange({ ...config, ...value });
  }
  const accountLimits = config.accountLimits ?? {};
  const accountLabels = config.accountLabels ?? {};
  const accounts = new Map((stats?.entries ?? []).map((entry) => [entry.key, entry]));
  for (const key of Object.keys(accountLimits)) {
    if (!accounts.has(key)) accounts.set(key, { key, profiles: [], active: 0, queued: 0, limit: accountLimits[key] });
  }
  return (
    <section className="space-y-4 border-t border-[var(--border-default)] pt-4" aria-label={i.admissionTitle}>
      <div>
        <h3 className="text-sm font-semibold">{i.admissionTitle}</h3>
        <p className="mt-1 text-xs text-[var(--text-secondary)]">{i.admissionDesc}</p>
      </div>
      <NumberRow
        label={i.admissionLimit}
        description={i.admissionLimitDesc}
        value={config.maxConcurrent}
        min={0}
        max={1000}
        onChange={(maxConcurrent) => patch({ maxConcurrent })}
      />
      <NumberRow
        label={i.admissionQueue}
        description={i.admissionQueueDesc}
        value={config.maxQueued}
        min={0}
        max={256}
        onChange={(maxQueued) => patch({ maxQueued })}
      />
      <NumberRow
        label={i.admissionWait}
        description={i.admissionWaitDesc}
        value={config.queueTimeoutSecs}
        min={1}
        max={600}
        onChange={(queueTimeoutSecs) => patch({ queueTimeoutSecs })}
      />
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Button
          variant="secondary"
          aria-expanded={expanded}
          onClick={() => {
            setExpanded(!expanded);
            if (!expanded) void refresh();
          }}
        >
          <ChevronDown size={14} className={expanded ? "rotate-180" : ""} />
          {i.admissionAccounts}
        </Button>
        {expanded && (
          <Button variant="ghost" onClick={() => void refresh()} disabled={loading}>
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
            {i.admissionRefresh}
          </Button>
        )}
      </div>
      {expanded && (
        <div className="space-y-3" aria-busy={loading}>
          {error && (
            <p role="alert" className="text-xs text-[var(--warning)]">
              {i.admissionReadFailed}
            </p>
          )}
          {loading && (
            <p role="status" className="text-xs text-[var(--text-muted)]">
              {i.admissionLoading}
            </p>
          )}
          {stats && (
            <p className="text-xs text-[var(--text-secondary)]">
              {i.admissionActive}: {stats.active} · {i.admissionQueued}: {stats.queued}
            </p>
          )}
          <p className="text-xs text-[var(--text-muted)]">{i.admissionAccountsDesc}</p>
          {[...accounts.values()].map((entry, index) => {
            const label =
              Array.from(entry.profiles.join(" / ")).slice(0, 128).join("") ||
              accountLabels[entry.key] ||
              `${i.admissionSavedAccount} ${index + 1}`;
            const overridden = Object.prototype.hasOwnProperty.call(accountLimits, entry.key);
            const liveEntry = stats?.entries.find((value) => value.key === entry.key);
            return (
              <div
                key={entry.key}
                className="space-y-2 rounded-md border border-[var(--border-default)] bg-[var(--bg-card)] p-3"
              >
                {liveEntry && (
                  <p className="text-xs text-[var(--text-muted)]">
                    {i.admissionApplied}: {liveEntry.limit === 0 ? i.admissionUnlimited : liveEntry.limit}
                  </p>
                )}
                <NumberRow
                  label={label}
                  description={`${stats ? `${i.admissionActive}: ${entry.active} · ${i.admissionQueued}: ${entry.queued}` : i.admissionUnknown} · ${i.admissionLimitDesc}`}
                  value={overridden ? accountLimits[entry.key] : config.maxConcurrent}
                  min={0}
                  max={1000}
                  onChange={(limit) =>
                    patch({
                      accountLimits: { ...accountLimits, [entry.key]: limit },
                      accountLabels: { ...accountLabels, [entry.key]: label },
                    })
                  }
                />
                {overridden ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      const limits = { ...accountLimits };
                      const labels = { ...accountLabels };
                      delete limits[entry.key];
                      delete labels[entry.key];
                      patch({ accountLimits: limits, accountLabels: labels });
                    }}
                  >
                    {i.admissionInherit}
                  </Button>
                ) : (
                  <p className="text-xs text-[var(--text-muted)]">{i.admissionInherited}</p>
                )}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
