import { memo, useCallback, useEffect, useState } from "react";
import { Copy, ExternalLink, KeyRound, Loader2, RefreshCw, Trash2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open as shellOpen } from "@tauri-apps/plugin-shell";

type LocaleText = (zh: string, en: string, ja?: string) => string;
interface XaiAccount {
  id: string;
  login: string;
  authenticatedAt: number;
  requiresReauth: boolean;
}
interface XaiStatus {
  accounts: XaiAccount[];
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
interface Props {
  localeText: LocaleText;
}

export default memo(function XaiOAuthAuthSection({ localeText }: Props) {
  const [status, setStatus] = useState<XaiStatus | null>(null);
  const [deviceCode, setDeviceCode] = useState<DeviceCode | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setStatus(await invoke<XaiStatus>("xai_oauth_get_status"));
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
        const account = await invoke<XaiAccount | null>("xai_oauth_poll_for_account", {
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
      const flow = await invoke<DeviceCode>("xai_oauth_start_device_flow");
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
        await invoke("xai_oauth_set_default_account", { accountId });
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
            "移除这个 xAI OAuth 账号？",
            "Remove this xAI OAuth account?",
            "この xAI OAuth アカウントを削除しますか？",
          ),
        )
      )
        return;
      setBusy(true);
      try {
        await invoke("xai_oauth_remove_account", { accountId });
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
          "退出全部 xAI OAuth 账号？",
          "Sign out all xAI OAuth accounts?",
          "すべての xAI OAuth アカウントからサインアウトしますか？",
        ),
      )
    )
      return;
    setBusy(true);
    try {
      await invoke("xai_oauth_logout");
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
          {localeText("xAI OAuth 认证", "xAI OAuth Auth", "xAI OAuth 認証")}
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
          "使用 xAI 设备码登录；令牌保存在系统凭据存储中。",
          "Sign in with an xAI device code; refresh tokens are kept in the system credential store.",
          "xAI デバイスコードでログインし、更新トークンはシステム資格情報ストアに保存します。",
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
              "打开 xAI 验证页并输入设备码，完成后此处会自动刷新。",
              "Open the xAI verification page and enter the device code; this panel polls automatically.",
              "xAI 認証ページを開いてデバイスコードを入力すると、自動で更新されます。",
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
            <div
              style={{
                color: account.requiresReauth ? "var(--warning)" : "var(--text-muted)",
                fontSize: 10,
                marginTop: 3,
              }}
            >
              {account.requiresReauth
                ? localeText("需要重新登录", "Re-authentication required", "再認証が必要")
                : account.id}
            </div>
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
