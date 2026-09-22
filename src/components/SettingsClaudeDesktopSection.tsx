import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle2, Monitor, RefreshCw, Upload } from "lucide-react";
import type { Locale } from "../lib/i18n";
import { Button } from "./ui/button";

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
    <section className="space-y-3" aria-label="Claude Desktop MCP">
      <div className="flex items-center gap-2 text-sm font-semibold">
        <Monitor size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "Claude Desktop 集成", "Claude Desktop Integration", "Claude Desktop 連携")}
      </div>
      {status === null ? (
        <div className="py-6 text-center text-xs text-[var(--text-muted)]">
          {uiText(locale, "读取中...", "Loading...", "読み込み中...")}
        </div>
      ) : !status.supported ? (
        <div className="py-6 text-center text-xs text-[var(--text-muted)]">
          {uiText(
            locale,
            "当前平台暂不支持自动定位配置文件",
            "Automatic config discovery is unavailable on this platform",
            "このプラットフォームでは設定ファイルを自動検出できません",
          )}
        </div>
      ) : (
        <>
          <div className="flex flex-wrap gap-x-8 gap-y-2 border-y border-[var(--border-default)] py-3 text-xs">
            <div>
              <span className="mr-2 text-[var(--text-muted)]">{uiText(locale, "配置状态", "Config", "設定")}</span>
              <span style={{ color: status.validJson ? "var(--success)" : "var(--danger)" }}>
                {status.configured && status.validJson
                  ? uiText(locale, "有效", "Valid", "有効")
                  : uiText(locale, "未配置或无效", "Missing or invalid", "未設定または無効")}
              </span>
            </div>
            <div>
              <span className="mr-2 text-[var(--text-muted)]">
                {uiText(locale, "MCP 服务", "MCP servers", "MCP サーバー")}
              </span>
              <span>{status.mcpServerCount}</span>
            </div>
          </div>
          <div className="break-all text-[11px] text-[var(--text-muted)]">{status.configPath}</div>
          <div className="flex flex-wrap gap-2">
            <Button variant="secondary" size="sm" onClick={() => void refresh()} disabled={busy}>
              <RefreshCw size={14} /> {uiText(locale, "刷新", "Refresh", "更新")}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void runAction("ensure_claude_desktop_official_provider")}
              disabled={busy}
            >
              <CheckCircle2 size={14} /> {uiText(locale, "校验并准备", "Validate & prepare", "検証して準備")}
            </Button>
            <Button
              size="sm"
              onClick={() => void runAction("import_claude_desktop_providers_from_claude")}
              disabled={busy}
            >
              <Upload size={14} /> {uiText(locale, "同步 Claude MCP", "Sync Claude MCP", "Claude MCP を同期")}
            </Button>
          </div>
        </>
      )}
      {message && (
        <div role="status" className="text-xs text-[var(--text-secondary)]">
          {message}
        </div>
      )}
    </section>
  );
}
