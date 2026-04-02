import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { Copy, ExternalLink, Github, Loader2, RefreshCw, Trash2 } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";

interface GitHubAccount {
  id: string;
  login: string;
  avatar_url: string | null;
  authenticated_at: number;
}

interface CopilotAuthStatus {
  accounts: GitHubAccount[];
  default_account_id: string | null;
  authenticated: boolean;
  username: string | null;
  expires_at: number | null;
}

interface GitHubDeviceCodeResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

interface CopilotAuthSectionProps {
  selectedAccountId?: string | null;
  onAccountSelect?: (accountId: string | null) => void;
  showDescription?: boolean;
}

function formatExpiry(unixSeconds: number | null) {
  if (!unixSeconds) return "";
  return new Date(unixSeconds * 1000).toLocaleString();
}

export default function CopilotAuthSection({
  selectedAccountId = null,
  onAccountSelect,
  showDescription = true,
}: CopilotAuthSectionProps) {
  const locale = getLocale();
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  );

  const [status, setStatus] = useState<CopilotAuthStatus | null>(null);
  const [deviceCode, setDeviceCode] = useState<GitHubDeviceCodeResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [starting, setStarting] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [defaultingAccountId, setDefaultingAccountId] = useState<string | null>(null);
  const [removingAccountId, setRemovingAccountId] = useState<string | null>(null);

  async function loadStatus(showError = false) {
    try {
      const next = await invoke<CopilotAuthStatus>("copilot_get_auth_status");
      setStatus(next);
    } catch (error) {
      if (showError) {
        showToast(
          "error",
          uiText(
            `读取 Copilot 认证状态失败: ${error}`,
            `Failed to load Copilot auth status: ${error}`,
            `Copilot 認証状態の読み込みに失敗しました: ${error}`,
          ),
        );
      }
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }

  useEffect(() => {
    void loadStatus();
  }, []);

  useEffect(() => {
    if (!deviceCode) return undefined;
    let cancelled = false;
    let timeoutId = 0;

    const poll = async () => {
      try {
        const account = await invoke<GitHubAccount | null>("copilot_poll_for_account", {
          deviceCode: deviceCode.device_code,
        });
        if (cancelled) return;
        if (account) {
          setDeviceCode(null);
          await loadStatus();
          showToast(
            "success",
            uiText(
              `GitHub Copilot 已授权：${account.login}`,
              `GitHub Copilot authorized: ${account.login}`,
              `GitHub Copilot を認証しました: ${account.login}`,
            ),
          );
          if (onAccountSelect) {
            onAccountSelect(account.id);
          }
          return;
        }
      } catch (error) {
        if (cancelled) return;
        setDeviceCode(null);
        showToast(
          "error",
          uiText(
            `GitHub 授权失败: ${error}`,
            `GitHub authorization failed: ${error}`,
            `GitHub 認証に失敗しました: ${error}`,
          ),
        );
        return;
      }

      timeoutId = window.setTimeout(
        () => void poll(),
        Math.max(deviceCode.interval, 2) * 1000,
      );
    };

    timeoutId = window.setTimeout(
      () => void poll(),
      Math.max(deviceCode.interval, 2) * 1000,
    );

    return () => {
      cancelled = true;
      window.clearTimeout(timeoutId);
    };
  }, [deviceCode, onAccountSelect]);

  async function startDeviceFlow() {
    setStarting(true);
    try {
      const flow = await invoke<GitHubDeviceCodeResponse>("copilot_start_device_flow");
      setDeviceCode(flow);
      try {
        await shellOpen(flow.verification_uri);
      } catch {
        // ignore browser-open failure; the user can open it manually
      }
    } catch (error) {
      showToast(
        "error",
        uiText(
          `启动 GitHub 授权失败: ${error}`,
          `Failed to start GitHub authorization: ${error}`,
          `GitHub 認証の開始に失敗しました: ${error}`,
        ),
      );
    } finally {
      setStarting(false);
    }
  }

  async function handleSetDefault(accountId: string) {
    setDefaultingAccountId(accountId);
    try {
      await invoke("copilot_set_default_account", { accountId });
      await loadStatus();
      showToast(
        "success",
        uiText("默认账号已更新", "Default account updated", "既定アカウントを更新しました"),
      );
      if (!selectedAccountId && onAccountSelect) {
        onAccountSelect(null);
      }
    } catch (error) {
      showToast(
        "error",
        uiText(
          `设置默认账号失败: ${error}`,
          `Failed to set default account: ${error}`,
          `既定アカウントの設定に失敗しました: ${error}`,
        ),
      );
    } finally {
      setDefaultingAccountId(null);
    }
  }

  async function handleRemoveAccount(accountId: string) {
    setRemovingAccountId(accountId);
    try {
      await invoke("copilot_remove_account", { accountId });
      await loadStatus();
      showToast(
        "success",
        uiText("账号已移除", "Account removed", "アカウントを削除しました"),
      );
      if (selectedAccountId === accountId && onAccountSelect) {
        onAccountSelect(null);
      }
    } catch (error) {
      showToast(
        "error",
        uiText(
          `移除账号失败: ${error}`,
          `Failed to remove account: ${error}`,
          `アカウント削除に失敗しました: ${error}`,
        ),
      );
    } finally {
      setRemovingAccountId(null);
    }
  }

  async function copyUserCode() {
    if (!deviceCode?.user_code) return;
    await navigator.clipboard.writeText(deviceCode.user_code);
    showToast(
      "success",
      uiText("授权码已复制", "Code copied", "コードをコピーしました"),
    );
  }

  if (loading) {
    return (
      <div className="card" style={{ padding: 16 }}>
        <div className="loading-center" style={{ minHeight: 120 }}>
          <div className="spinner" />
        </div>
      </div>
    );
  }

  const accounts = status?.accounts || [];
  const defaultAccountId = status?.default_account_id || null;

  return (
    <div className="card" style={{ padding: 16, display: "flex", flexDirection: "column", gap: 14 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
        <div>
          <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 14, fontWeight: 700 }}>
            <Github size={16} />
            {uiText("GitHub Copilot 认证", "GitHub Copilot Auth", "GitHub Copilot 認証")}
          </div>
          {showDescription && (
            <div style={{ marginTop: 6, fontSize: 12, color: "var(--text-muted)", lineHeight: 1.6 }}>
              {uiText(
                "支持 Device Code 登录、多账号保存、默认账号切换，以及给 Provider 绑定指定 GitHub Copilot 账号。",
                "Supports Device Code sign-in, multiple saved accounts, default-account switching, and per-provider GitHub Copilot binding.",
                "Device Code ログイン、複数アカウント保存、既定アカウント切替、Provider ごとの GitHub Copilot 紐付けに対応します。",
              )}
            </div>
          )}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="btn btn-secondary btn-sm"
            type="button"
            onClick={() => {
              setRefreshing(true);
              void loadStatus(true);
            }}
            disabled={refreshing}
            style={{ gap: 6 }}
          >
            {refreshing ? <Loader2 size={14} className="spin" /> : <RefreshCw size={14} />}
            {uiText("刷新", "Refresh", "更新")}
          </button>
          <button
            className="btn btn-primary btn-sm"
            type="button"
            onClick={() => void startDeviceFlow()}
            disabled={starting}
            style={{ gap: 6 }}
          >
            {starting ? <Loader2 size={14} className="spin" /> : <Github size={14} />}
            {accounts.length > 0
              ? uiText("添加账号", "Add Account", "アカウント追加")
              : uiText("GitHub 登录", "Sign In", "GitHub でログイン")}
          </button>
        </div>
      </div>

      <div style={{ display: "flex", gap: 10, flexWrap: "wrap", fontSize: 12 }}>
        <span className={`badge ${accounts.length > 0 ? "badge-success" : "badge-muted"}`}>
          {accounts.length > 0
            ? uiText(`已授权 ${accounts.length} 个账号`, `${accounts.length} account(s) connected`, `${accounts.length} 件のアカウント接続済み`)
            : uiText("未授权", "Not connected", "未接続")}
        </span>
        {status?.expires_at ? (
          <span className="badge badge-muted">
            {uiText("默认 Token 过期", "Default token expires", "既定 Token 期限")}: {formatExpiry(status.expires_at)}
          </span>
        ) : null}
      </div>

      {onAccountSelect && (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <label className="field-label">
            {uiText("Provider 绑定账号", "Provider Account Binding", "Provider の紐付けアカウント")}
          </label>
          <select
            className="input"
            value={selectedAccountId || ""}
            onChange={(event) => onAccountSelect(event.target.value || null)}
            style={{ fontSize: 13 }}
          >
            <option value="">{uiText("使用默认账号", "Use default account", "既定アカウントを使用")}</option>
            {accounts.map((account) => (
              <option key={account.id} value={account.id}>
                {account.login}
              </option>
            ))}
          </select>
        </div>
      )}

      {deviceCode && (
        <div style={{ padding: 14, borderRadius: 10, background: "var(--bg-elevated)", border: "1px solid var(--border-color)" }}>
          <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText(
              "浏览器打开 GitHub 授权页后，输入下面的设备码完成登录。",
              "Open the GitHub verification page and enter the device code below.",
              "GitHub の認証ページを開き、以下のデバイスコードを入力してください。",
            )}
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
            <code className="badge badge-accent" style={{ fontSize: 18, padding: "10px 14px", fontFamily: "'JetBrains Mono', monospace" }}>
              {deviceCode.user_code}
            </code>
            <button className="btn btn-secondary btn-sm" type="button" onClick={() => void copyUserCode()} style={{ gap: 6 }}>
              <Copy size={14} />
              {uiText("复制", "Copy", "コピー")}
            </button>
            <button className="btn btn-secondary btn-sm" type="button" onClick={() => void shellOpen(deviceCode.verification_uri)} style={{ gap: 6 }}>
              <ExternalLink size={14} />
              {uiText("打开授权页", "Open Browser", "ブラウザで開く")}
            </button>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => setDeviceCode(null)}>
              {uiText("取消", "Cancel", "キャンセル")}
            </button>
          </div>
        </div>
      )}

      {accounts.length === 0 ? (
        <div style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.7 }}>
          {uiText(
            "还没有可用的 GitHub Copilot 账号。完成登录后，Claude Provider 就可以绑定这个账号并通过本地代理发起请求。",
            "No GitHub Copilot accounts are connected yet. After sign-in, Claude provider profiles can bind one of these accounts and send requests through the local proxy.",
            "利用可能な GitHub Copilot アカウントはまだありません。ログイン後、Claude Provider からこのアカウントを紐付けてローカルプロキシ経由で利用できます。",
          )}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          {accounts.map((account) => {
            const isDefault = account.id === defaultAccountId;
            const isSelected = selectedAccountId === account.id;
            return (
              <div
                key={account.id}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  gap: 12,
                  alignItems: "center",
                  padding: "12px 14px",
                  borderRadius: 10,
                  background: "var(--bg-input)",
                }}
              >
                <div style={{ minWidth: 0 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                    <span style={{ fontSize: 13, fontWeight: 600 }}>{account.login}</span>
                    {isDefault ? <span className="badge badge-success">{uiText("默认", "Default", "既定")}</span> : null}
                    {isSelected ? <span className="badge badge-accent">{uiText("当前绑定", "Bound", "現在の紐付け")}</span> : null}
                  </div>
                  <div style={{ marginTop: 4, fontSize: 11, color: "var(--text-muted)" }}>
                    ID: {account.id}
                  </div>
                </div>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap", justifyContent: "flex-end" }}>
                  {!isDefault ? (
                    <button
                      className="btn btn-secondary btn-sm"
                      type="button"
                      onClick={() => void handleSetDefault(account.id)}
                      disabled={defaultingAccountId === account.id}
                    >
                      {defaultingAccountId === account.id
                        ? uiText("设置中...", "Saving...", "保存中...")
                        : uiText("设为默认", "Set Default", "既定に設定")}
                    </button>
                  ) : null}
                  <button
                    className="btn btn-danger-ghost btn-sm"
                    type="button"
                    onClick={() => void handleRemoveAccount(account.id)}
                    disabled={removingAccountId === account.id}
                    style={{ gap: 6 }}
                  >
                    {removingAccountId === account.id ? <Loader2 size={14} className="spin" /> : <Trash2 size={14} />}
                    {uiText("移除", "Remove", "削除")}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
