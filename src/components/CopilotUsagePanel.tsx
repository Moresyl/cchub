import { useCallback, useEffect, useState } from "react";
import { BarChart3, RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

type LocaleText = (zh: string, en: string, ja?: string) => string;
interface Quota {
  entitlement: number;
  remaining: number;
  percent_remaining: number;
  unlimited: boolean;
}
interface Usage {
  copilot_plan: string;
  quota_reset_date: string;
  quota_snapshots: { chat: Quota; completions: Quota; premium_interactions: Quota };
}
interface Model {
  id: string;
  name: string;
  vendor: string;
}

interface Props {
  localeText: LocaleText;
}
interface AuthStatus {
  authenticated: boolean;
}

function QuotaItem({ label, value }: { label: string; value: Quota }) {
  const percent = Math.max(0, Math.min(100, value.percent_remaining));
  return (
    <div style={{ display: "grid", gap: 4 }}>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11 }}>
        <span>{label}</span>
        <span style={{ color: "var(--text-muted)" }}>
          {value.unlimited ? "∞" : `${value.remaining}/${value.entitlement}`}
        </span>
      </div>
      <div style={{ height: 5, borderRadius: 4, background: "var(--border)" }}>
        <div
          style={{
            height: "100%",
            width: `${percent}%`,
            borderRadius: 4,
            background: percent < 20 ? "var(--danger)" : "var(--accent)",
          }}
        />
      </div>
    </div>
  );
}

export default function CopilotUsagePanel({ localeText }: Props) {
  const [usage, setUsage] = useState<Usage | null>(null);
  const [models, setModels] = useState<Model[]>([]);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");
  const refresh = useCallback(async () => {
    setLoading(true);
    setMessage("");
    try {
      const auth = await invoke<AuthStatus>("copilot_get_auth_status");
      if (!auth.authenticated) {
        setUsage(null);
        setModels([]);
        return;
      }
      const [nextUsage, nextModels] = await Promise.all([
        invoke<Usage>("copilot_get_usage"),
        invoke<Model[]>("copilot_get_models"),
      ]);
      setUsage(nextUsage);
      setModels(nextModels);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => {
    void refresh();
  }, [refresh]);
  return (
    <div style={{ marginTop: 12, border: "1px solid var(--border)", borderRadius: 8, padding: 10 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, marginBottom: 8 }}>
        <span style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, fontWeight: 600 }}>
          <BarChart3 size={14} />
          {localeText("Copilot 配额与模型", "Copilot quota and models", "Copilot クォータとモデル")}
        </span>
        <button className="btn btn-ghost btn-icon-sm" onClick={() => void refresh()} disabled={loading}>
          <RefreshCw size={13} />
        </button>
      </div>
      {message ? (
        <div style={{ color: "var(--text-muted)", fontSize: 11 }}>{message}</div>
      ) : usage ? (
        <>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 8 }}>
            <span className="badge badge-accent">{usage.copilot_plan}</span>
            <span className="badge badge-muted">
              {localeText("重置", "Reset", "リセット")}: {usage.quota_reset_date}
            </span>
          </div>
          <div style={{ display: "grid", gap: 8 }}>
            <QuotaItem
              label={localeText("Premium", "Premium", "Premium")}
              value={usage.quota_snapshots.premium_interactions}
            />
            <QuotaItem label={localeText("聊天", "Chat", "チャット")} value={usage.quota_snapshots.chat} />
            <QuotaItem label={localeText("补全", "Completions", "補完")} value={usage.quota_snapshots.completions} />
          </div>
          <div style={{ color: "var(--text-muted)", fontSize: 10, marginTop: 8 }}>
            {localeText(
              `可用模型 ${models.length} 个`,
              `${models.length} models available`,
              `${models.length} モデル利用可能`,
            )}
          </div>
        </>
      ) : (
        <div style={{ color: "var(--text-muted)", fontSize: 11 }}>
          {localeText(
            "登录 Copilot 后可查询配额。",
            "Sign in to Copilot to query quota.",
            "Copilot にログインするとクォータを確認できます。",
          )}
        </div>
      )}
    </div>
  );
}
