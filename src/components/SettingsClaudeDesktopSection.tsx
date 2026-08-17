import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle2, Monitor, RefreshCw, Upload } from "lucide-react";
import type { Locale } from "../lib/i18n";

interface ClaudeDesktopStatus {
  supported: boolean;
  configured: boolean;
  validJson: boolean;
  configPath?: string;
  mcpServerCount: number;
}

interface ClaudeDesktopImportResult {
  imported: number;
  updated: number;
  skipped: number;
  configPath: string;
}

interface SettingsClaudeDesktopSectionProps {
  locale: Locale;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

export default function SettingsClaudeDesktopSection({ locale }: SettingsClaudeDesktopSectionProps) {
  const [status, setStatus] = useState<ClaudeDesktopStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke<ClaudeDesktopStatus>("get_claude_desktop_status"));
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const runAction = useCallback(
    async (command: string) => {
      setBusy(true);
      setMessage(null);
      try {
        const result = await invoke<ClaudeDesktopStatus | ClaudeDesktopImportResult>(command);
        if ("imported" in result) {
          setMessage(
            uiText(
              locale,
              `已导入 ${result.imported} 个、更新 ${result.updated} 个 MCP 服务`,
              `Imported ${result.imported} and updated ${result.updated} MCP servers`,
              `MCP サーバーを ${result.imported} 件追加、${result.updated} 件更新しました`,
            ),
          );
        } else {
          setMessage(
            uiText(
              locale,
              "Claude Desktop 配置已准备好",
              "Claude Desktop configuration is ready",
              "Claude Desktop の設定を準備しました",
            ),
          );
        }
        setStatus(await invoke<ClaudeDesktopStatus>("get_claude_desktop_status"));
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [locale],
  );

  return (
    <div className="section-card">
      <div className="section-card-title">
        <Monitor size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "Claude Desktop 集成", "Claude Desktop Integration", "Claude Desktop 連携")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
        {uiText(
          locale,
          "检测桌面配置、校验 JSON，并将 Claude Code 的 MCP 服务安全同步到桌面客户端。",
          "Inspect the desktop config, validate JSON, and safely sync MCP servers from Claude Code.",
          "デスクトップ設定を検査し、JSON を検証して Claude Code の MCP サーバーを安全に同期します。",
        )}
      </p>
      {!status?.supported ? (
        <div className="empty-state">
          {uiText(
            locale,
            "当前平台暂不支持自动定位配置文件",
            "Automatic config discovery is unavailable on this platform",
            "このプラットフォームでは設定ファイルを自動検出できません",
          )}
        </div>
      ) : (
        <>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
              gap: 10,
              marginBottom: 12,
            }}
          >
            <div className="stat-card">
              <div className="stat-card-label">{uiText(locale, "配置状态", "Config", "設定")}</div>
              <div className="stat-card-value" style={{ color: status.validJson ? "var(--success)" : "var(--danger)" }}>
                {status.configured && status.validJson
                  ? uiText(locale, "有效", "Valid", "有効")
                  : uiText(locale, "未配置或无效", "Missing or invalid", "未設定または無効")}
              </div>
            </div>
            <div className="stat-card">
              <div className="stat-card-label">{uiText(locale, "MCP 服务", "MCP servers", "MCP サーバー")}</div>
              <div className="stat-card-value">{status.mcpServerCount}</div>
            </div>
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 12, wordBreak: "break-all" }}>
            {status.configPath}
          </div>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button className="btn btn-secondary btn-sm" onClick={() => void refresh()} disabled={busy}>
              <RefreshCw size={14} /> {uiText(locale, "刷新", "Refresh", "更新")}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => void runAction("ensure_claude_desktop_official_provider")}
              disabled={busy}
            >
              <CheckCircle2 size={14} /> {uiText(locale, "校验并准备", "Validate & prepare", "検証して準備")}
            </button>
            <button
              className="btn btn-primary btn-sm"
              onClick={() => void runAction("import_claude_desktop_providers_from_claude")}
              disabled={busy}
            >
              <Upload size={14} /> {uiText(locale, "同步 Claude MCP", "Sync Claude MCP", "Claude MCP を同期")}
            </button>
          </div>
        </>
      )}
      {message && <div style={{ marginTop: 10, fontSize: 12, color: "var(--text-secondary)" }}>{message}</div>}
    </div>
  );
}
