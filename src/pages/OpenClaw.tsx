import { useEffect, useId, useState, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Bot, RefreshCw, Settings2, Shield, Wrench } from "lucide-react";
import { t } from "../lib/i18n";
import { useAsyncResource } from "../lib/asyncState";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import EmptyState from "../components/states/EmptyState";
import { Button } from "../components/ui/button";
import EnvPanel from "./OpenClaw/EnvPanel";
import ToolsPanel from "./OpenClaw/ToolsPanel";
import AgentsPanel from "./OpenClaw/AgentsPanel";

interface OpenClawStatus {
  installed: boolean;
  configPath: string;
}
interface HealthWarning {
  code: string;
  message: string;
  path?: string | null;
}
type Tab = "env" | "tools" | "agents";

export default function OpenClaw() {
  const i = t().openClaw;
  const id = useId();
  const [tab, setTab] = useState<Tab>("env");
  const [opened, setOpened] = useState(false);
  const status = useAsyncResource(() => invoke<OpenClawStatus>("get_openclaw_status"));
  const health = useAsyncResource(() => invoke<HealthWarning[]>("scan_openclaw_health"));
  useEffect(() => {
    if (status.data?.installed) setOpened(true);
  }, [status.data?.installed]);
  const tabs = [
    { key: "env" as const, icon: Settings2, label: i.envTab, panel: EnvPanel },
    { key: "tools" as const, icon: Wrench, label: i.toolsTab, panel: ToolsPanel },
    { key: "agents" as const, icon: Shield, label: i.agentsTab, panel: AgentsPanel },
  ];
  function navigateTabs(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const next =
      event.key === "ArrowRight"
        ? (index + 1) % tabs.length
        : event.key === "ArrowLeft"
          ? (index + tabs.length - 1) % tabs.length
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? tabs.length - 1
              : null;
    if (next === null) return;
    event.preventDefault();
    setTab(tabs[next].key);
    event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
  }
  if (!status.data)
    return status.loading ? (
      <LoadingState label={i.loading} />
    ) : (
      <ErrorState title={i.readFailed} message={i.readFailedDesc} retryLabel={i.retry} onRetry={status.reload} />
    );
  const installed = status.data.installed;
  if (!installed && !opened)
    return (
      <EmptyState
        title={i.notInstalled}
        description={i.notInstalledDesc}
        icon={<Bot size={26} />}
        action={
          <Button variant="secondary" disabled={status.loading} onClick={status.reload}>
            <RefreshCw size={14} aria-hidden="true" />
            {i.checkAgain}
          </Button>
        }
      />
    );
  return (
    <div className="page-enter flex min-h-full min-w-0 flex-col">
      <header className="page-header">
        <div className="min-w-0">
          <h2 className="page-title">{i.title}</h2>
          <p className="page-subtitle">{i.subtitle}</p>
          <p className="mt-2 break-all text-xs text-[var(--text-muted)]">{status.data.configPath}</p>
        </div>
        <Button
          variant="secondary"
          disabled={status.loading || health.loading}
          onClick={() => {
            status.reload();
            health.reload();
          }}
        >
          <RefreshCw size={14} aria-hidden="true" />
          {i.checkAgain}
        </Button>
      </header>
      {status.error && (
        <ErrorState title={i.readFailed} message={i.readFailedDesc} retryLabel={i.retry} onRetry={status.reload} />
      )}
      {!installed && (
        <p
          role="alert"
          className="mb-4 rounded-lg border border-border bg-card p-3 text-xs text-[var(--text-secondary)]"
        >
          {i.configMissing}
        </p>
      )}
      {health.error && (
        <div
          role="alert"
          className="mb-4 flex flex-wrap items-center gap-3 rounded-lg border border-border bg-card p-3"
        >
          <AlertTriangle size={16} className="shrink-0 text-[var(--warning)]" aria-hidden="true" />
          <p className="min-w-0 flex-1 basis-48 text-xs text-[var(--text-secondary)]">{i.healthFailed}</p>
          <Button variant="secondary" disabled={health.loading} onClick={health.reload}>
            {i.retryHealth}
          </Button>
        </div>
      )}
      {health.data && health.data.length > 0 && (
        <div className="mb-4 space-y-2">
          {health.data.map((warning, index) => (
            <div
              key={`${warning.code}-${index}`}
              className="flex items-start gap-2 rounded-lg border border-[var(--border-default)] bg-[var(--warning-subtle)] p-3 text-xs"
            >
              <AlertTriangle size={15} className="shrink-0 text-[var(--warning)]" aria-hidden="true" />
              <div className="min-w-0 flex-1 break-words text-[var(--text-secondary)]">
                <p>{warning.message}</p>
                {warning.path && <p className="mt-1 break-all font-mono text-[var(--text-muted)]">{warning.path}</p>}
              </div>
            </div>
          ))}
        </div>
      )}
      <div className="entity-detail-tabs mb-5 !px-0" role="tablist" aria-label={i.title}>
        {tabs.map(({ key, icon: Icon, label }, index) => (
          <Button
            key={key}
            variant="ghost"
            role="tab"
            id={`${id}-${key}-tab`}
            aria-controls={`${id}-${key}-panel`}
            aria-selected={tab === key}
            tabIndex={tab === key ? 0 : -1}
            className={`entity-detail-tab ${tab === key ? "entity-detail-tab-active" : ""}`}
            onClick={() => setTab(key)}
            onKeyDown={(event) => navigateTabs(event, index)}
          >
            <Icon size={14} aria-hidden="true" />
            {label}
          </Button>
        ))}
      </div>
      {tabs.map(({ key, panel: Panel }) => (
        <div
          key={key}
          role="tabpanel"
          id={`${id}-${key}-panel`}
          aria-labelledby={`${id}-${key}-tab`}
          hidden={tab !== key}
          className="min-w-0"
        >
          <Panel blocked={status.loading || Boolean(status.error) || !installed} />
        </div>
      ))}
    </div>
  );
}
