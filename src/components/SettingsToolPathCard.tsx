import { memo, type FocusEvent } from "react";
import { Check, Copy, FolderOpen } from "lucide-react";
import type { Locale } from "../lib/i18n";

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
  onSaveMcpPath: (toolId: string, value: string, defaultValue: string, customPath?: SettingsToolPathCardCustomPath) => void | Promise<void>;
  onPickMcpPath: (toolId: string, customPath?: SettingsToolPathCardCustomPath) => void | Promise<void>;
  onSaveSkillsDir: (toolId: string, value: string, defaultValue: string, customPath?: SettingsToolPathCardCustomPath) => void | Promise<void>;
  onPickSkillsDir: (toolId: string, customPath?: SettingsToolPathCardCustomPath) => void | Promise<void>;
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
  const handleMcpBlur = (event: FocusEvent<HTMLInputElement>) => {
    void onSaveMcpPath(tool.id, event.target.value, tool.mcp_config_path, customPath);
  };

  const handleSkillsBlur = (event: FocusEvent<HTMLInputElement>) => {
    void onSaveSkillsDir(tool.id, event.target.value, tool.skills_dir, customPath);
  };

  return (
    <div style={{ padding: "12px 16px", borderRadius: 8, background: "var(--bg-input)", opacity: tool.installed ? 1 : 0.6 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>{tool.name}</span>
          <span className={`badge ${tool.installed ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
            {tool.installed ? uiText(locale, "已安装", "Installed", "インストール済み") : uiText(locale, "未安装", "Not installed", "未インストール")}
          </span>
        </div>
        {saved && <Check size={14} style={{ color: "var(--success)" }} />}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <span style={{ fontSize: 11, color: "var(--text-muted)", width: 80, flexShrink: 0 }}>MCP</span>
          <input
            className="input"
            style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace", padding: "4px 8px", height: 28, flex: 1 }}
            defaultValue={customPath?.mcp_config_path || tool.mcp_config_path}
            onBlur={handleMcpBlur}
          />
          <button
            className="btn btn-ghost btn-icon-sm"
            title={uiText(locale, "选择文件", "Pick file", "ファイルを選択")}
            onClick={() => void onPickMcpPath(tool.id, customPath)}
          >
            <FolderOpen size={12} />
          </button>
        </div>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <span style={{ fontSize: 11, color: "var(--text-muted)", width: 80, flexShrink: 0 }}>Skills</span>
          <input
            className="input"
            style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace", padding: "4px 8px", height: 28, flex: 1 }}
            defaultValue={customPath?.skills_dir || tool.skills_dir}
            onBlur={handleSkillsBlur}
          />
          <button
            className="btn btn-ghost btn-icon-sm"
            title={uiText(locale, "选择文件夹", "Pick folder", "フォルダーを選択")}
            onClick={() => void onPickSkillsDir(tool.id, customPath)}
          >
            <FolderOpen size={12} />
          </button>
        </div>
      </div>
      {!tool.installed && tool.install_command && (
        <div style={{ marginTop: 8, display: "flex", alignItems: "center", gap: 6 }}>
          <code style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>{tool.install_command}</code>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => void onCopyInstallCommand(tool.install_command, tool.name)}
            title="Copy"
          >
            <Copy size={11} />
          </button>
        </div>
      )}
    </div>
  );
}

export default memo(SettingsToolPathCardComponent);
