import { memo, useCallback, useEffect, useState } from "react";
import { Copy, ExternalLink, KeyRound, Loader2, RefreshCw, Trash2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open as shellOpen } from "@tauri-apps/plugin-shell";

type LocaleText = (zh: string, en: string, ja?: string) => string;

interface CodexAccount {
  id: string;
  login: string;
  authenticatedAt: number;
}
interface CodexStatus {
  accounts: CodexAccount[];
  defaultAccountId?: string | null;
  authenticated: boolean;
  username?: string | null;
}
interface DeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}
interface AccountQuotaTier {
  name: string;
  utilization: number;
  resetsAt?: string | null;
}
interface AccountQuota {
  success: boolean;
  tiers: AccountQuotaTier[];
  error?: string | null;
}
interface Props {
  localeText: LocaleText;
}

function AccountQuota({ accountId, localeText }: { accountId: string; localeText: LocaleText }) {
  const [quota, setQuota] = useState<AccountQuota | null>(null);
  useEffect(() => {
    let active = true;
    void invoke<AccountQuota>("get_codex_oauth_quota", { accountId })
      .then((value) => {
        if (active) setQuota(value);
      })
      .catch(() => {
        if (active) setQuota(null);
      });
    return () => {
      active = false;
    };
  }, [accountId]);
  if (!quota)
    return (
      <span style={{ color: "var(--text-muted)", fontSize: 10 }}>
        {localeText("配额读取中...", "Loading quota...", "クォータを読み込み中...")}
      </span>
    );
  if (!quota.success || quota.tiers.length === 0)
    return (
      <span style={{ color: "var(--text-muted)", fontSize: 10 }}>
        {localeText("暂无配额", "Quota unavailable", "クォータなし")}
      </span>
    );
  return (
    <div style={{ display: "grid", gap: 3, minWidth: 180 }}>
      {quota.tiers.map((tier) => {
        const utilization = Math.max(0, Math.min(100, tier.utilization));
        const color = utilization >= 90 ? "var(--danger)" : utilization >= 70 ? "var(--warning)" : "var(--accent)";
        return (
          <div
            key={tier.name}
            title={tier.resetsAt ? `${tier.name}: ${tier.resetsAt}` : tier.name}
            style={{ display: "flex", alignItems: "center", gap: 6 }}
          >
            <span style={{ width: 58, fontSize: 10, color: "var(--text-muted)" }}>{tier.name.replace("_", " ")}</span>
            <div style={{ height: 4, flex: 1, minWidth: 55, background: "var(--border)", borderRadius: 3 }}>
              <div style={{ height: "100%", width: `${utilization}%`, background: color, borderRadius: 3 }} />
            </div>
            <span style={{ width: 36, textAlign: "right", fontSize: 10, color: "var(--text-muted)" }}>
              {utilization.toFixed(0)}%
            </span>
          </div>
        );
      })}
    </div>
  );
}

export default memo(function CodexOAuthAuthSection({ localeText }: Props) {
  const [status, setStatus] = useState<CodexStatus | null>(null);
  const [deviceCode, setDeviceCode] = useState<DeviceCode | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setStatus(await invoke<CodexStatus>("codex_oauth_get_status"));
      setMessage("");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!deviceCode) return undefined;
    let cancelled = false;
    let timer: number | undefined;
    const poll = async () => {
      try {
        const account = await invoke<CodexAccount | null>("codex_oauth_poll_for_account", {
          deviceCode: deviceCode.deviceCode,
        });
        if (cancelled) return;
        if (account) {
          setDeviceCode(null);
          await load();
          setMessage(
            localeText(`已授权 ${account.login}`, `Authorized ${account.login}`, `${account.login} を認証しました`),
          );
          return;
        }
      } catch (error) {
        if (!cancelled && !String(error).toLowerCase().includes("pending")) {
          setDeviceCode(null);
          setMessage(String(error));
          return;
        }
      }
      if (!cancelled) timer = window.setTimeout(() => void poll(), Math.max(2, deviceCode.interval) * 1000);
    };
    timer = window.setTimeout(() => void poll(), Math.max(2, deviceCode.interval) * 1000);
    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [deviceCode, load, localeText]);

  const startLogin = useCallback(async () => {
    setBusy(true);
    setMessage("");
    try {
      const flow = await invoke<DeviceCode>("codex_oauth_start_device_flow");
      setDeviceCode(flow);
      try {
        await shellOpen(flow.verificationUri);
      } catch {
        /* manual open remains available */
      }
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, []);

  const setDefault = useCallback(
    async (accountId: string) => {
      setBusy(true);
      try {
        await invoke("codex_oauth_set_default_account", { accountId });
        await load();
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [load],
  );

  const remove = useCallback(
    async (accountId: string) => {
      if (
        !window.confirm(
          localeText(
            "移除这个 Codex OAuth 账号？",
            "Remove this Codex OAuth account?",
            "この Codex OAuth アカウントを削除しますか？",
          ),
        )
      )
        return;
      setBusy(true);
      try {
        await invoke("codex_oauth_remove_account", { accountId });
        await load();
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [load, localeText],
  );

  const logout = useCallback(async () => {
    if (
      !window.confirm(
        localeText(
          "退出全部 Codex OAuth 账号？",
          "Sign out all Codex OAuth accounts?",
          "すべての Codex OAuth アカウントからサインアウトしますか？",
        ),
      )
    )
      return;
    setBusy(true);
    try {
      await invoke("codex_oauth_logout");
      setDeviceCode(null);
      await load();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }, [load, localeText]);

  const copyCode = useCallback(async () => {
    if (!deviceCode) return;
    await navigator.clipboard.writeText(deviceCode.userCode);
    setMessage(localeText("设备码已复制", "Device code copied", "デバイスコードをコピーしました"));
  }, [deviceCode, localeText]);

  if (loading)
    return (
      <div className="card" style={{ padding: 14 }}>
        <div className="loading-center" style={{ minHeight: 80 }}>
          <div className="spinner" />
        </div>
      </div>
    );
  const accounts = status?.accounts ?? [];
  return (
    <div className="card" style={{ padding: 14, display: "grid", gap: 12 }}>
      <div
        style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "center", flexWrap: "wrap" }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 7, fontWeight: 700, fontSize: 14 }}>
          <KeyRound size={16} />
          {localeText("Codex OAuth 认证", "Codex OAuth Auth", "Codex OAuth 認証")}
        </div>
        <div style={{ display: "flex", gap: 7 }}>
          <button className="btn btn-secondary btn-sm" onClick={() => void load()} disabled={busy}>
            <RefreshCw size={13} />
            {localeText("刷新", "Refresh", "更新")}
          </button>
          <button className="btn btn-primary btn-sm" onClick={() => void startLogin()} disabled={busy}>
            <KeyRound size={13} />
            {accounts.length
              ? localeText("添加账号", "Add account", "アカウント追加")
              : localeText("登录", "Sign in", "ログイン")}
          </button>
          {accounts.length > 0 && (
            <button className="btn btn-ghost btn-sm" onClick={() => void logout()} disabled={busy}>
              {localeText("退出全部", "Sign out all", "すべて退出")}
            </button>
          )}
        </div>
      </div>
      <div style={{ color: "var(--text-muted)", fontSize: 11 }}>
        {localeText(
          "使用设备码登录；令牌仅用于本地请求，账号列表不包含密钥。",
          "Sign in with a device code; tokens are used locally and account metadata never includes secrets.",
          "デバイスコードでログインします。トークンはローカル要求にのみ使用し、アカウント一覧に秘密情報は含めません。",
        )}
      </div>
      <div>
        <span className={`badge ${accounts.length ? "badge-success" : "badge-muted"}`}>
          {accounts.length
            ? localeText(
                `已连接 ${accounts.length} 个账号`,
                `${accounts.length} account(s) connected`,
                `${accounts.length} 件接続済み`,
              )
            : localeText("未连接", "Not connected", "未接続")}
        </span>
      </div>
      {deviceCode && (
        <div style={{ padding: 10, border: "1px solid var(--border)", borderRadius: 8, display: "grid", gap: 8 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {localeText(
              "打开验证页并输入设备码，完成后此处会自动刷新。",
              "Open the verification page and enter the device code; this panel polls automatically.",
              "認証ページを開いてデバイスコードを入力すると、自動で更新されます。",
            )}
          </div>
          <div style={{ display: "flex", gap: 7, alignItems: "center", flexWrap: "wrap" }}>
            <code className="badge badge-accent" style={{ fontSize: 17, letterSpacing: ".08em" }}>
              {deviceCode.userCode}
            </code>
            <button className="btn btn-secondary btn-sm" onClick={() => void copyCode()}>
              <Copy size={13} />
              {localeText("复制", "Copy", "コピー")}
            </button>
            <button className="btn btn-secondary btn-sm" onClick={() => void shellOpen(deviceCode.verificationUri)}>
              <ExternalLink size={13} />
              {localeText("打开验证页", "Open", "開く")}
            </button>
            <button className="btn btn-ghost btn-sm" onClick={() => setDeviceCode(null)}>
              {localeText("取消", "Cancel", "キャンセル")}
            </button>
          </div>
        </div>
      )}
      {message && <div style={{ color: "var(--text-muted)", fontSize: 11 }}>{message}</div>}
      {accounts.map((account) => (
        <div
          key={account.id}
          style={{
            display: "flex",
            justifyContent: "space-between",
            gap: 10,
            alignItems: "center",
            padding: "9px 10px",
            background: "var(--bg-input)",
            borderRadius: 8,
          }}
        >
          <div style={{ minWidth: 0 }}>
            <div style={{ fontWeight: 600, fontSize: 12 }}>{account.login}</div>
            <div style={{ color: "var(--text-muted)", fontSize: 10, marginTop: 3 }}>{account.id}</div>
            <AccountQuota accountId={account.id} localeText={localeText} />
          </div>
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap", justifyContent: "flex-end" }}>
            {account.id === status?.defaultAccountId ? (
              <span className="badge badge-success">{localeText("默认", "Default", "既定")}</span>
            ) : (
              <button className="btn btn-secondary btn-xs" onClick={() => void setDefault(account.id)} disabled={busy}>
                {localeText("设为默认", "Set default", "既定に設定")}
              </button>
            )}
            <button
              className="btn btn-danger-ghost btn-icon-sm"
              onClick={() => void remove(account.id)}
              disabled={busy}
              title={localeText("移除", "Remove", "削除")}
            >
              <Trash2 size={13} />
            </button>
          </div>
        </div>
      ))}
      {busy && <Loader2 size={14} className="spin" style={{ color: "var(--text-muted)" }} />}
    </div>
  );
});
