import { memo, useCallback, useEffect, useRef, useState } from "react";
import { Copy, ExternalLink, KeyRound, Loader2, RefreshCw, Trash2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { useAppDialog } from "./AppDialogProvider";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
import AccountQuota from "./CodexOAuthAuthSection/AccountQuota";

type LocaleText = (zh: string, en: string, ja?: string) => string;

interface CodexAccount {
  id: string;
  login: string;
  authenticatedAt: number;
  requiresReauth?: boolean;
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
interface Props {
  localeText: LocaleText;
}

export default memo(function CodexOAuthAuthSection({ localeText }: Props) {
  const appDialog = useAppDialog();
  const [status, setStatus] = useState<CodexStatus | null>(null);
  const [deviceCode, setDeviceCode] = useState<DeviceCode | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [quotaEpoch, setQuotaEpoch] = useState(0);
  const owner = useRef({ generation: 0, healthGeneration: 0, mounted: true });

  const load = useCallback(async () => {
    const generation = ++owner.current.generation;
    setLoading(true);
    try {
      const next = await invoke<CodexStatus>("codex_oauth_get_status");
      if (!owner.current.mounted || generation !== owner.current.generation) return;
      setStatus(next);
      setQuotaEpoch((epoch) => epoch + 1);
      setMessage("");
    } catch (error) {
      if (owner.current.mounted && generation === owner.current.generation) setMessage(String(error));
    } finally {
      if (owner.current.mounted && generation === owner.current.generation) setLoading(false);
    }
  }, []);

  useEffect(() => {
    const active = owner.current;
    active.mounted = true;
    void load();
    return () => {
      active.mounted = false;
      ++active.generation;
    };
  }, [load]);

  const refreshHealth = useCallback(async () => {
    const generation = owner.current.generation;
    const healthGeneration = ++owner.current.healthGeneration;
    try {
      const next = await invoke<CodexStatus>("codex_oauth_get_status");
      if (
        owner.current.mounted &&
        generation === owner.current.generation &&
        healthGeneration === owner.current.healthGeneration
      )
        setStatus(next);
    } catch {
      /* The explicit refresh action remains available. */
    }
  }, []);

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

  const cancelLogin = useCallback(async () => {
    if (!deviceCode) return;
    setDeviceCode(null);
    try {
      await invoke("codex_oauth_cancel_device_flow", { deviceCode: deviceCode.deviceCode });
    } catch (error) {
      setMessage(String(error));
    }
  }, [deviceCode]);

  const remove = useCallback(
    async (accountId: string) => {
      const confirmed = await appDialog.confirm({
        title: localeText("移除 OAuth 账号", "Remove OAuth account", "OAuth アカウントを削除"),
        message: localeText(
          "此账号将从 CCHub 中移除，需要时可重新登录。",
          "This account will be removed from CCHub. You can sign in again later.",
          "このアカウントを CCHub から削除します。後で再ログインできます。",
        ),
        confirmText: localeText("移除", "Remove", "削除"),
        cancelText: localeText("取消", "Cancel", "キャンセル"),
        tone: "danger",
      });
      if (!confirmed) return;
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
    [appDialog, load, localeText],
  );

  const logout = useCallback(async () => {
    const confirmed = await appDialog.confirm({
      title: localeText("退出全部账号", "Sign out all accounts", "すべてのアカウントからログアウト"),
      message: localeText(
        "所有 Codex OAuth 登录状态都将从本机移除。",
        "All Codex OAuth sessions will be removed from this device.",
        "すべての Codex OAuth セッションをこの端末から削除します。",
      ),
      confirmText: localeText("全部退出", "Sign out all", "すべてログアウト"),
      cancelText: localeText("取消", "Cancel", "キャンセル"),
      tone: "danger",
    });
    if (!confirmed) return;
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
  }, [appDialog, load, localeText]);

  const copyCode = useCallback(async () => {
    if (!deviceCode) return;
    await navigator.clipboard.writeText(deviceCode.userCode);
    setMessage(localeText("设备码已复制", "Device code copied", "デバイスコードをコピーしました"));
  }, [deviceCode, localeText]);

  if (loading && !status)
    return (
      <Card className="flex min-h-24 items-center justify-center" role="status">
        <Loader2 size={16} className="animate-spin" aria-hidden="true" />
        <span className="sr-only">{localeText("正在读取账号…", "Loading accounts…", "アカウントを読み込み中…")}</span>
      </Card>
    );
  const accounts = status?.accounts ?? [];
  return (
    <Card className="min-w-0 space-y-4 p-4" aria-busy={busy || loading}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h3 className="flex items-center gap-2 text-sm font-semibold">
          <KeyRound size={16} aria-hidden="true" />
          {localeText("Codex OAuth 认证", "Codex OAuth Auth", "Codex OAuth 認証")}
        </h3>
        <div className="flex flex-wrap gap-2">
          <Button variant="secondary" onClick={() => void load()} disabled={busy || loading}>
            <RefreshCw size={13} />
            {localeText("刷新", "Refresh", "更新")}
          </Button>
          <Button onClick={() => void startLogin()} disabled={busy || !!deviceCode}>
            <KeyRound size={13} />
            {accounts.length
              ? localeText("添加账号", "Add account", "アカウント追加")
              : localeText("登录", "Sign in", "ログイン")}
          </Button>
          {accounts.length > 0 && (
            <Button variant="ghost" onClick={() => void logout()} disabled={busy}>
              {localeText("退出全部", "Sign out all", "すべて退出")}
            </Button>
          )}
        </div>
      </div>
      <p className="text-xs leading-relaxed text-muted-foreground">
        {localeText(
          "使用设备码登录；令牌仅用于本地请求，账号列表不包含密钥。",
          "Sign in with a device code; tokens are used locally and account metadata never includes secrets.",
          "デバイスコードでログインします。トークンはローカル要求にのみ使用し、アカウント一覧に秘密情報は含めません。",
        )}
      </p>
      <div>
        <span className="text-xs text-muted-foreground">
          {accounts.length
            ? localeText(
                `已保存 ${accounts.length} 个账号`,
                `${accounts.length} saved account(s)`,
                `${accounts.length} 件保存済み`,
              )
            : localeText("未连接", "Not connected", "未接続")}
        </span>
      </div>
      {deviceCode && (
        <div className="space-y-3 rounded-lg border border-border p-3">
          <p className="text-xs leading-relaxed text-muted-foreground">
            {localeText(
              "打开验证页并输入设备码，完成后此处会自动刷新。",
              "Open the verification page and enter the device code; this panel polls automatically.",
              "認証ページを開いてデバイスコードを入力すると、自動で更新されます。",
            )}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <code className="rounded-md bg-secondary px-3 py-2 text-base tracking-wider">{deviceCode.userCode}</code>
            <Button variant="secondary" onClick={() => void copyCode()}>
              <Copy size={13} />
              {localeText("复制", "Copy", "コピー")}
            </Button>
            <Button variant="secondary" onClick={() => void shellOpen(deviceCode.verificationUri)}>
              <ExternalLink size={13} />
              {localeText("打开验证页", "Open", "開く")}
            </Button>
            <Button variant="ghost" onClick={() => void cancelLogin()}>
              {localeText("取消", "Cancel", "キャンセル")}
            </Button>
          </div>
        </div>
      )}
      {message && (
        <p role="status" className="break-words text-xs text-muted-foreground">
          {message}
        </p>
      )}
      {accounts.map((account) => (
        <div
          key={account.id}
          className="flex min-w-0 flex-wrap items-start justify-between gap-3 rounded-lg border border-border bg-secondary/40 p-3"
        >
          <div className="min-w-0 flex-[1_1_240px] space-y-2">
            <p className="break-words text-sm font-medium">{account.login}</p>
            <p className="break-all text-[11px] text-muted-foreground">{account.id}</p>
            {account.requiresReauth ? (
              <p className="text-xs text-[var(--warning)]">
                {localeText(
                  "授权已失效，请重新登录此账号。",
                  "Authorization expired. Sign in to this account again.",
                  "認証が失効しました。このアカウントに再ログインしてください。",
                )}
              </p>
            ) : (
              <AccountQuota
                key={`${account.id}:${account.authenticatedAt}`}
                accountId={account.id}
                refreshKey={quotaEpoch}
                localeText={localeText}
                onFailure={refreshHealth}
              />
            )}
          </div>
          <div className="flex max-w-full flex-wrap items-center justify-end gap-2">
            {account.requiresReauth && (
              <Button variant="secondary" disabled={busy || !!deviceCode} onClick={() => void startLogin()}>
                {localeText("重新登录", "Sign in again", "再ログイン")}
              </Button>
            )}
            {account.id === status?.defaultAccountId ? (
              <span className="badge badge-success">{localeText("默认", "Default", "既定")}</span>
            ) : (
              <Button
                variant="secondary"
                onClick={() => void setDefault(account.id)}
                disabled={busy || account.requiresReauth}
              >
                {localeText("设为默认", "Set default", "既定に設定")}
              </Button>
            )}
            <Button
              variant="ghost"
              size="icon"
              className="text-[var(--danger)]"
              onClick={() => void remove(account.id)}
              disabled={busy}
              title={localeText("移除", "Remove", "削除")}
              aria-label={`${localeText("移除", "Remove", "削除")} ${account.login}`}
            >
              <Trash2 size={13} />
            </Button>
          </div>
        </div>
      ))}
      {busy && <Loader2 size={14} className="animate-spin text-muted-foreground" aria-hidden="true" />}
    </Card>
  );
});
