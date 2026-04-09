import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle, Download, RefreshCw } from "lucide-react";
import { t } from "../lib/i18n";
import UpdateCard, { type UpdateCardInfo } from "../components/UpdateCard";

type UpdateInfo = UpdateCardInfo;

export default function Updates() {
  const [updates, setUpdates] = useState<UpdateInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const i = t();

  const check = useCallback(async () => {
    setLoading(true);
    try { setUpdates(await invoke<UpdateInfo[]>("check_updates")); }
    catch (e) { console.error(e); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { void check(); }, [check]);

  const handleCheck = useCallback(() => {
    void check();
  }, [check]);

  if (loading) {
    return <div className="loading-center"><div className="spinner" /><span style={{ fontSize: 13, color: "var(--text-muted)" }}>{i.updates.checking}</span></div>;
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.updates.title}</h2>
        </div>
        <div style={{ display: "flex", gap: 10 }}>
          {updates.length > 1 && (
            <button className="btn btn-primary btn-sm"><Download size={14} />{i.updates.updateAll}</button>
          )}
          <button className="btn btn-secondary btn-sm" onClick={handleCheck}><RefreshCw size={14} />{i.updates.checkNow}</button>
        </div>
      </div>

      {updates.length === 0 ? (
        <div className="card empty-state" style={{ flex: 1 }}>
          <div className="empty-icon" style={{ background: "var(--success-subtle)", borderColor: "transparent" }}>
            <CheckCircle size={28} style={{ color: "var(--success)" }} />
          </div>
          <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)" }}>{i.updates.allUpToDate}</p>
          <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8, maxWidth: 320 }}>{i.updates.allUpToDateTip}</p>
        </div>
      ) : (
        <div style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: 10 }} className="stagger">
          {updates.map((update) => (
            <UpdateCard
              key={update.item_id}
              update={update}
              updateLabel={i.updates.update}
            />
          ))}
        </div>
      )}
    </div>
  );
}
