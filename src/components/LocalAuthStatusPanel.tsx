import { useCallback, useEffect, useState } from "react";
import { CheckCircle2, KeyRound, RefreshCw, XCircle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

type LocaleText = (zh: string, en: string, ja?: string) => string;
interface LocalAuthStatus {
  toolId: string;
  authenticated: boolean;
  source: string;
  credentialPath?: string | null;
  detail: string;
}

const LABELS: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  grokbuild: "Grok Build",
  opencode: "OpenCode",
  openclaw: "OpenClaw",
  hermes: "Hermes",
  pi: "Pi",
};

interface Props {
  localeText: LocaleText;
}

export default function LocalAuthStatusPanel({ localeText }: Props) {
  const [statuses, setStatuses] = useState<LocalAuthStatus[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      setStatuses(await invoke<LocalAuthStatus[]>("get_local_auth_status"));
    } catch (value) {
      setError(String(value));
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);
  return (
    <div style={{ marginTop: 14, borderTop: "1px solid var(--border)", paddingTop: 12 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
        <span style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, fontWeight: 600 }}>
          <KeyRound size={14} />
          {localeText("本机认证状态", "Local credential status", "ローカル認証状態")}
        </span>
        <button
          className="btn btn-ghost btn-icon-sm"
          onClick={() => void load()}
          disabled={loading}
          title={localeText("刷新", "Refresh", "更新")}
        >
          <RefreshCw size={14} />
        </button>
      </div>
      <p style={{ color: "var(--text-muted)", fontSize: 11, marginBottom: 8 }}>
        {localeText(
          "仅检查凭据是否存在，不会读取或显示密钥。",
          "Only checks whether a credential exists; secrets are never displayed.",
          "資格情報の存在だけを確認し、秘密情報は表示しません。",
        )}
      </p>
      {error && <div style={{ color: "var(--danger)", fontSize: 11 }}>{error}</div>}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 6 }}>
        {statuses.map((status) => (
          <div
            key={status.toolId}
            style={{ border: "1px solid var(--border)", borderRadius: 6, padding: "7px 8px", minWidth: 0 }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: 5, fontSize: 12 }}>
              <span
                style={{ color: status.authenticated ? "var(--success)" : "var(--text-muted)", display: "inline-flex" }}
              >
                {status.authenticated ? <CheckCircle2 size={13} /> : <XCircle size={13} />}
              </span>
              <span>{LABELS[status.toolId] || status.toolId}</span>
            </div>
            <div style={{ color: "var(--text-muted)", fontSize: 10, marginTop: 3 }}>
              {status.authenticated ? status.source : localeText("未检测到", "Not detected", "未検出")}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
