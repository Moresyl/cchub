import type { ReactNode } from "react";
import { Loader2, RotateCcw, Save } from "lucide-react";
import { t } from "../../lib/i18n";
import { Button } from "../../components/ui/button";
import ErrorState from "../../components/states/ErrorState";
import LoadingState from "../../components/states/LoadingState";

interface ConfigPanelProps {
  title: string;
  description?: string;
  ready: boolean;
  loading: boolean;
  loadError: boolean;
  saveFailed: boolean;
  saving: boolean;
  blocked: boolean;
  dirty: boolean;
  issue?: string | null;
  reload: () => void;
  reset: () => void;
  save: () => void;
  children: ReactNode;
}

export default function ConfigPanel(props: ConfigPanelProps) {
  const i = t().openClaw;
  if (!props.ready)
    return props.loading ? (
      <LoadingState label={i.loading} />
    ) : (
      <ErrorState title={i.readFailed} message={i.readFailedDesc} retryLabel={i.retry} onRetry={props.reload} />
    );
  return (
    <form
      className="mx-auto w-full max-w-[760px]"
      onSubmit={(event) => {
        event.preventDefault();
        props.save();
      }}
    >
      {props.loadError && (
        <ErrorState title={i.readFailed} message={i.readFailedDesc} retryLabel={i.retry} onRetry={props.reload} />
      )}
      <fieldset
        aria-label={props.title}
        disabled={props.saving || props.loading || props.loadError || props.blocked}
        aria-busy={props.saving || props.loading}
        className="m-0 min-w-0 space-y-5 border-0 p-0"
      >
        <div>
          <h3 className="text-sm font-semibold">{props.title}</h3>
          {props.description && (
            <p className="mt-1 text-xs leading-5 text-[var(--text-secondary)]">{props.description}</p>
          )}
        </div>
        {props.children}
        {(props.issue || props.saveFailed) && (
          <p role="alert" className="text-xs text-[var(--danger)]">
            {props.issue || i.saveFailedDesc}
          </p>
        )}
        <div className="sticky bottom-0 z-10 flex flex-wrap items-center gap-2 border-t border-border bg-[var(--bg-app)] py-3">
          <p role="status" className="min-w-0 flex-1 text-xs text-[var(--text-muted)]">
            {props.dirty ? i.unsaved : i.saved}
          </p>
          <Button type="button" variant="ghost" disabled={!props.dirty} onClick={props.reset}>
            <RotateCcw size={14} aria-hidden="true" />
            {i.reset}
          </Button>
          <Button type="submit" disabled={!props.dirty || Boolean(props.issue)}>
            {props.saving ? (
              <Loader2 size={14} className="animate-spin" aria-hidden="true" />
            ) : (
              <Save size={14} aria-hidden="true" />
            )}
            {props.saving ? i.saving : i.save}
          </Button>
        </div>
      </fieldset>
    </form>
  );
}
