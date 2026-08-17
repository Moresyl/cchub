import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle2, Code2, Loader2, RefreshCw, ShieldCheck } from "lucide-react";
import { showToast } from "./Toast";
import { getLocale } from "../lib/i18n";

interface ClaudeExtensionStatus {
  path: string;
  exists: boolean;
  enabled: boolean;
  validJson: boolean;
}

const text = (locale: string, zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);

export default function SettingsClaudeExtensionSection() {
  const locale = getLocale();
  const [integrationEnabled, setIntegrationEnabled] = useState(false);
  const [status, setStatus] = useState<ClaudeExtensionStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [enabled, nextStatus] = await Promise.all([
        invoke<boolean>("get_claude_extension_integration"),
        invoke<ClaudeExtensionStatus>("get_claude_extension_status"),
      ]);
      setIntegrationEnabled(enabled);
      setStatus(nextStatus);
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const toggleIntegration = useCallback(async () => {
    setBusy(true);
    try {
      const next = await invoke<boolean>("set_claude_extension_integration", {
        enabled: !integrationEnabled,
      });
      setIntegrationEnabled(next);
      showToast(
        "success",
        next
          ? text(
              locale,
              "已开启 Claude Code 扩展随配置切换",
              "Claude Code extension sync enabled",
              "Claude Code 拡張機能の同期を有効にしました",
            )
          : text(
              locale,
              "已关闭 Claude Code 扩展随配置切换",
              "Claude Code extension sync disabled",
              "Claude Code 拡張機能の同期を無効にしました",
            ),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setBusy(false);
    }
  }, [integrationEnabled, locale]);

  const apply = useCallback(
    async (official: boolean) => {
      setBusy(true);
      try {
        const next = await invoke<ClaudeExtensionStatus>("apply_claude_extension_config", { official });
        setStatus(next);
        showToast(
          "success",
          official
            ? text(
                locale,
                "已恢复 Claude Code 官方配置",
                "Claude Code official configuration restored",
                "Claude Code の公式設定に戻しました",
              )
            : text(
                locale,
                "已应用 Claude Code 托管配置",
                "Claude Code managed configuration applied",
                "Claude Code の管理設定を適用しました",
              ),
        );
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setBusy(false);
      }
    },
    [locale],
  );

  return (
    <section className="section-card" aria-labelledby="claude-extension-title">
      <div className="section-card-title" id="claude-extension-title">
        <Code2 size={17} style={{ color: "var(--text-secondary)" }} />
        {text(locale, "Claude Code 扩展集成", "Claude Code extension integration", "Claude Code 拡張機能の連携")}
      </div>
      <p style={{ color: "var(--text-muted)", fontSize: 12, lineHeight: 1.55, margin: "0 0 14px" }}>
        {text(
          locale,
          "可选地让 Claude Code 扩展跟随 CCHub 切换配置。更新只修改 primaryApiKey，其他扩展设置会保留。",
          "Optionally let the Claude Code extension follow CCHub profile switches. Updates only touch primaryApiKey and preserve other extension settings.",
          "Claude Code 拡張機能を CCHub のプロファイル切替に追従させます。primaryApiKey のみ更新し、他の設定は保持します。",
        )}
      </p>
      {loading ? (
        <div style={{ display: "flex", alignItems: "center", gap: 8, color: "var(--text-muted)" }}>
          <Loader2 size={15} className="spin" />
          {text(locale, "正在读取扩展状态…", "Loading extension status…", "拡張機能の状態を読み込み中…")}
        </div>
      ) : (
        <>
          <label style={{ display: "flex", alignItems: "flex-start", gap: 10, cursor: busy ? "wait" : "pointer" }}>
            <input
              type="checkbox"
              checked={integrationEnabled}
              onChange={() => void toggleIntegration()}
              disabled={busy}
            />
            <span>
              <span style={{ display: "block", fontSize: 13, color: "var(--text-primary)" }}>
                {text(
                  locale,
                  "切换 Claude profile 时同步扩展",
                  "Sync the extension when switching Claude profiles",
                  "Claude プロファイル切替時に拡張機能を同期",
                )}
              </span>
              <span style={{ display: "block", marginTop: 3, fontSize: 11, color: "var(--text-muted)" }}>
                {text(
                  locale,
                  "官方 profile 会移除托管覆盖，其他 profile 会启用托管入口。",
                  "Official profiles remove the managed override; other profiles enable the managed entry point.",
                  "公式プロファイルでは管理上書きを削除し、それ以外では管理エントリを有効にします。",
                )}
              </span>
            </span>
          </label>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
              gap: 10,
              marginTop: 14,
            }}
          >
            <div className="stat-card">
              <div className="stat-card-label">{text(locale, "配置状态", "Config", "設定")}</div>
              <div
                className="stat-card-value"
                style={{ color: status?.validJson ? "var(--success)" : "var(--danger)" }}
              >
                {status?.validJson
                  ? status.exists
                    ? text(locale, "有效", "Valid", "有効")
                    : text(locale, "尚未创建", "Not created", "未作成")
                  : text(locale, "JSON 无效", "Invalid JSON", "JSON が無効")}
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-card-label">{text(locale, "托管覆盖", "Managed override", "管理上書き")}</div>
              <div
                className="stat-card-value"
                style={{ color: status?.enabled ? "var(--accent)" : "var(--text-secondary)" }}
              >
                {status?.enabled
                  ? text(locale, "已启用", "Enabled", "有効")
                  : text(locale, "未启用", "Disabled", "無効")}
              </div>
            </div>
          </div>
          <div
            style={{
              display: "flex",
              alignItems: "flex-start",
              gap: 7,
              marginTop: 10,
              fontSize: 11,
              color: "var(--text-muted)",
              wordBreak: "break-all",
            }}
          >
            <ShieldCheck size={13} style={{ flexShrink: 0, marginTop: 1 }} />
            {status?.path}
          </div>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginTop: 12 }}>
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => void load()} disabled={busy}>
              <RefreshCw size={14} /> {text(locale, "刷新", "Refresh", "更新")}
            </button>
            <button
              type="button"
              className="btn btn-primary btn-sm"
              onClick={() => void apply(false)}
              disabled={busy || !status?.validJson}
            >
              <CheckCircle2 size={14} /> {text(locale, "应用托管配置", "Apply managed", "管理設定を適用")}
            </button>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => void apply(true)}
              disabled={busy || !status?.validJson}
            >
              {text(locale, "恢复官方配置", "Restore official", "公式設定に戻す")}
            </button>
          </div>
        </>
      )}
    </section>
  );
}
