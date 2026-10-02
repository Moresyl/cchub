import { AlertCircle, Loader2, RefreshCw } from "lucide-react";
import { Button } from "../../components/ui/button";

interface SettingsNoticeProps {
  title: string;
  message: string;
  reloadLabel: string;
  loading: boolean;
  reload: () => void;
}

export default function SettingsNotice({ title, message, reloadLabel, loading, reload }: SettingsNoticeProps) {
  return (
    <div
      role="alert"
      className="mx-5 my-3 flex shrink-0 flex-wrap items-start gap-3 rounded-lg border border-[var(--border-default)] bg-[var(--bg-card)] p-3"
    >
      <AlertCircle size={16} className="mt-0.5 shrink-0 text-[var(--danger)]" aria-hidden="true" />
      <div className="min-w-0 flex-1 basis-48">
        <p className="text-sm font-semibold text-[var(--text-primary)]">{title}</p>
        <p className="mt-1 text-xs text-[var(--text-secondary)]">{message}</p>
      </div>
      <Button variant="secondary" disabled={loading} onClick={reload}>
        {loading ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
        {reloadLabel}
      </Button>
    </div>
  );
}
