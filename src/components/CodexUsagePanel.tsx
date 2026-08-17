import { useCallback, useEffect, useState } from "react";
import { BarChart3, RefreshCw } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

type LocaleText = (zh: string, en: string, ja?: string) => string;

interface QuotaTier {
  name: string;
  utilization: number;
  resetsAt?: string | null;
}

interface CodexQuota {
  credentialStatus: string;
  credentialMessage?: string | null;
  success: boolean;
  tiers: QuotaTier[];
  error?: string | null;
}

interface CodexModel {
  id: string;
  displayName?: string | null;
}

interface Props {
  localeText: LocaleText;
}

function tierLabel(name: string, localeText: LocaleText) {
  const labels: Record<string, [string, string, string]> = {
    five_hour: ["5 小时窗口", "5-hour window", "5時間ウィンドウ"],
    seven_day: ["7 天窗口", "7-day window", "7日間ウィンドウ"],
    thirty_day: ["30 天窗口", "30-day window", "30日間ウィンドウ"],
  };
  const label = labels[name];
  return label ? localeText(...label) : name;
}

function TierItem({ tier, localeText }: { tier: QuotaTier; localeText: LocaleText }) {
  const utilization = Math.max(0, Math.min(100, tier.utilization));
  const color = utilization >= 90 ? "var(--danger)" : utilization >= 70 ? "var(--warning)" : "var(--accent)";
  return (
    <div style={{ display: "grid", gap: 4 }}>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11 }}>
        <span>{tierLabel(tier.name, localeText)}</span>
        <span style={{ color: "var(--text-muted)" }}>{utilization.toFixed(1)}%</span>
      </div>
      <div style={{ height: 5, borderRadius: 4, background: "var(--border)" }}>
        <div style={{ height: "100%", width: `${utilization}%`, borderRadius: 4, background: color }} />
      </div>
      {tier.resetsAt && (
        <span style={{ color: "var(--text-muted)", fontSize: 10 }}>
          {localeText("重置", "Reset", "リセット")}: {tier.resetsAt}
        </span>
      )}
    </div>
  );
}

export default function CodexUsagePanel({ localeText }: Props) {
  const [quota, setQuota] = useState<CodexQuota | null>(null);
  const [claudeQuota, setClaudeQuota] = useState<CodexQuota | null>(null);
  const [models, setModels] = useState<CodexModel[]>([]);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true);
    setMessage("");
    const [quotaResult, modelResult, claudeResult] = await Promise.allSettled([
      invoke<CodexQuota>("get_codex_cli_quota"),
      invoke<CodexModel[]>("get_codex_cli_models"),
      invoke<CodexQuota>("get_claude_cli_quota"),
    ]);
    if (quotaResult.status === "fulfilled") {
      setQuota(quotaResult.value);
      if (quotaResult.value.error) setMessage(quotaResult.value.error);
    } else {
      setQuota(null);
      setMessage(String(quotaResult.reason));
    }
    if (modelResult.status === "fulfilled") {
      setModels(modelResult.value);
    } else {
      setModels([]);
    }
    if (claudeResult.status === "fulfilled") setClaudeQuota(claudeResult.value);
    else setClaudeQuota(null);
    setLoading(false);
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const hasQuota = Boolean(quota?.success && quota.tiers.length);
  const hasClaudeQuota = Boolean(claudeQuota?.success && claudeQuota.tiers.length);
  return (
    <div style={{ marginTop: 12, border: "1px solid var(--border)", borderRadius: 8, padding: 10 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, marginBottom: 8 }}>
        <span style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, fontWeight: 600 }}>
          <BarChart3 size={14} />
          {localeText("官方 OAuth 配额与模型", "Official OAuth quota and models", "公式 OAuth クォータとモデル")}
        </span>
        <button
          className="btn btn-ghost btn-icon-sm"
          onClick={() => void refresh()}
          disabled={loading}
          title={localeText("刷新", "Refresh", "更新")}
        >
          <RefreshCw size={13} />
        </button>
      </div>
      {message && <div style={{ color: "var(--text-muted)", fontSize: 11, marginBottom: 8 }}>{message}</div>}
      {hasQuota && (
        <div style={{ display: "grid", gap: 8 }}>
          {quota?.tiers.map((tier) => (
            <TierItem key={tier.name} tier={tier} localeText={localeText} />
          ))}
        </div>
      )}
      {quota?.credentialStatus === "not_found" && (
        <div style={{ color: "var(--text-muted)", fontSize: 11 }}>
          {localeText(
            "未检测到 Codex OAuth 登录。",
            "No Codex OAuth login detected.",
            "Codex OAuth ログインが検出されませんでした。",
          )}
        </div>
      )}
      {quota?.credentialStatus === "parse_error" && (
        <div style={{ color: "var(--text-muted)", fontSize: 11 }}>
          {localeText(
            "Codex 凭据文件无法解析，请使用 Codex CLI 重新登录。",
            "The Codex credential file could not be parsed; re-login with the Codex CLI.",
            "Codex 資格情報を解析できません。Codex CLI で再ログインしてください。",
          )}
        </div>
      )}
      {models.length > 0 && (
        <div style={{ color: "var(--text-muted)", fontSize: 10, marginTop: 8 }}>
          {localeText(
            `可用模型 ${models.length} 个`,
            `${models.length} models available`,
            `${models.length} モデル利用可能`,
          )}
        </div>
      )}
      {!loading && quota?.success && quota.tiers.length === 0 && (
        <div style={{ color: "var(--text-muted)", fontSize: 11 }}>
          {localeText(
            "已登录，但服务端未返回可用窗口。",
            "Signed in, but no quota windows were returned.",
            "ログイン済みですが、クォータのウィンドウが返されませんでした。",
          )}
        </div>
      )}
      {(hasClaudeQuota || claudeQuota?.credentialStatus === "expired") && (
        <div style={{ borderTop: "1px solid var(--border)", marginTop: 10, paddingTop: 10 }}>
          <div style={{ fontSize: 11, fontWeight: 600, marginBottom: 7 }}>
            {localeText("Claude OAuth 配额", "Claude OAuth quota", "Claude OAuth クォータ")}
          </div>
          {hasClaudeQuota ? (
            <div style={{ display: "grid", gap: 8 }}>
              {claudeQuota?.tiers.map((tier) => (
                <TierItem key={tier.name} tier={tier} localeText={localeText} />
              ))}
            </div>
          ) : (
            <div style={{ color: "var(--warning)", fontSize: 11 }}>
              {localeText(
                "Claude OAuth 凭据已过期，请在 Claude CLI 中重新登录。",
                "Claude OAuth credentials have expired; re-login with the Claude CLI.",
                "Claude OAuth 資格情報の期限が切れています。Claude CLI で再ログインしてください。",
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
