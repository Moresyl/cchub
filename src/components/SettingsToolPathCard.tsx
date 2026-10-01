import { memo, useRef, useState } from "react";
import { Check, Copy, Loader2 } from "lucide-react";
import type { Locale } from "../lib/i18n";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import SettingsToolPathInput from "./SettingsToolPathInput";

export interface SettingsToolPathCardTool {
  id: string;
  name: string;
  mcp_config_path: string;
  skills_dir: string;
  installed: boolean;
  install_command: string;
}

export interface SettingsToolPathCardCustomPath {
  tool_id: string;
  config_dir: string | null;
  mcp_config_path: string | null;
  skills_dir: string | null;
}

interface SettingsToolPathCardProps {
  tool: SettingsToolPathCardTool;
  customPath?: SettingsToolPathCardCustomPath;
  locale: Locale;
  saved: boolean;
  onSaveMcpPath: (
    toolId: string,
    value: string,
    defaultValue: string,
    customPath?: SettingsToolPathCardCustomPath,
  ) => string | Promise<string>;
  onPickMcpPath: (toolId: string, customPath?: SettingsToolPathCardCustomPath) => Promise<string | null>;
  onSaveSkillsDir: (
    toolId: string,
    value: string,
    defaultValue: string,
    customPath?: SettingsToolPathCardCustomPath,
  ) => string | Promise<string>;
  onPickSkillsDir: (toolId: string, customPath?: SettingsToolPathCardCustomPath) => Promise<string | null>;
  onCopyInstallCommand: (command: string, toolName: string) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsToolPathCardComponent({
  tool,
  customPath,
  locale,
  saved,
  onSaveMcpPath,
  onPickMcpPath,
  onSaveSkillsDir,
  onPickSkillsDir,
  onCopyInstallCommand,
}: SettingsToolPathCardProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const running = useRef(false);
  const runAction = async <T,>(action: () => T | Promise<T>): Promise<T | undefined> => {
    if (running.current) return undefined;
    running.current = true;
    setBusy(true);
    setError(false);
    try {
      return await action();
    } catch {
      setError(true);
      return undefined;
    } finally {
      running.current = false;
      setBusy(false);
    }
  };

  return (
    <Card className="min-w-0" aria-busy={busy}>
      <CardHeader className="flex-row flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <CardTitle className="break-words text-[14px] leading-snug">{tool.name}</CardTitle>
          <span className={`badge ${tool.installed ? "badge-success" : "badge-muted"}`}>
            {tool.installed
              ? uiText(locale, "已安装", "Installed", "インストール済み")
              : uiText(locale, "未安装", "Not installed", "未インストール")}
          </span>
        </div>
        {busy ? (
          <Loader2
            size={14}
            className="animate-spin motion-reduce:animate-none text-muted-foreground"
            aria-hidden="true"
          />
        ) : (
          saved && (
            <span role="status" className="flex items-center gap-1 text-[12px] text-[var(--success)]">
              <Check size={14} aria-hidden="true" />
              {uiText(locale, "已保存", "Saved", "保存済み")}
            </span>
          )
        )}
      </CardHeader>
      <CardContent className="space-y-4">
        <SettingsToolPathInput
          label="MCP"
          value={customPath?.mcp_config_path || tool.mcp_config_path}
          pickerLabel={uiText(locale, "选择 MCP 配置文件", "Pick MCP configuration file", "MCP 設定ファイルを選択")}
          busy={busy}
          onSave={(value) => runAction(() => onSaveMcpPath(tool.id, value, tool.mcp_config_path, customPath))}
          onPick={() => runAction(() => onPickMcpPath(tool.id, customPath))}
        />
        <SettingsToolPathInput
          label="Skills"
          value={customPath?.skills_dir || tool.skills_dir}
          pickerLabel={uiText(locale, "选择 Skills 文件夹", "Pick Skills folder", "Skills フォルダーを選択")}
          busy={busy}
          onSave={(value) => runAction(() => onSaveSkillsDir(tool.id, value, tool.skills_dir, customPath))}
          onPick={() => runAction(() => onPickSkillsDir(tool.id, customPath))}
        />
        {error && (
          <p role="alert" className="text-[12px] leading-relaxed text-[var(--danger)]">
            {uiText(
              locale,
              "路径操作失败，修改已保留，请重试。",
              "Path action failed. Your edits are preserved; please retry.",
              "パス操作に失敗しました。編集内容は保持されています。再試行してください。",
            )}
          </p>
        )}
        {!tool.installed && tool.install_command && (
          <div className="flex min-w-0 items-start gap-2 rounded-md bg-[var(--bg-input)] p-3">
            <code className="min-w-0 flex-1 break-all font-mono text-[12px] leading-relaxed text-muted-foreground">
              {tool.install_command}
            </code>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => void runAction(() => onCopyInstallCommand(tool.install_command, tool.name))}
              disabled={busy}
              aria-label={uiText(locale, "复制安装命令", "Copy install command", "インストールコマンドをコピー")}
              title={uiText(locale, "复制安装命令", "Copy install command", "インストールコマンドをコピー")}
            >
              <Copy size={14} aria-hidden="true" />
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default memo(SettingsToolPathCardComponent);
