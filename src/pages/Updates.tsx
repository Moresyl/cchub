import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle, Download, RefreshCw } from "lucide-react";
import { t } from "../lib/i18n";
import UpdateCard, { type UpdateCardInfo } from "../components/UpdateCard";
import EmptyState from "../components/states/EmptyState";
import ErrorState from "../components/states/ErrorState";
import LoadingState from "../components/states/LoadingState";

type UpdateInfo = UpdateCardInfo;

export default function Updates() {
  const [updates, setUpdates] = useState<UpdateInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const i = t();

  const check = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setUpdates(await invoke<UpdateInfo[]>("check_updates"));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void check();
  }, [check]);

  const handleCheck = useCallback(() => {
    void check();
  }, [check]);

  if (loading) {
    return <LoadingState label={i.updates.checking} />;
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.updates.title}</h2>
        </div>
        <div style={{ display: "flex", gap: 10 }}>
          {updates.length > 1 && (
            <button className="btn btn-primary btn-sm">
              <Download size={14} />
              {i.updates.updateAll}
            </button>
          )}
          <button className="btn btn-secondary btn-sm" onClick={handleCheck}>
            <RefreshCw size={14} />
            {i.updates.checkNow}
          </button>
        </div>
      </div>

      {error ? (
        <ErrorState title={i.updates.checkNow} message={error} retryLabel={i.updates.checkNow} onRetry={handleCheck} />
      ) : updates.length === 0 ? (
        <EmptyState
          title={i.updates.allUpToDate}
          description={i.updates.allUpToDateTip}
          icon={<CheckCircle size={28} style={{ color: "var(--success)" }} />}
        />
      ) : (
        <div
          style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 10 }}
          className="stagger"
        >
          {updates.map((update) => (
            <UpdateCard key={update.item_id} update={update} updateLabel={i.updates.update} />
          ))}
        </div>
      )}
    </div>
  );
}
