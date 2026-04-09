import { memo } from "react";
import { FolderOpen } from "lucide-react";
import type { Locale } from "../lib/i18n";
import SettingsToolPathCard from "./SettingsToolPathCard";

interface SettingsToolPathSectionCustomPath {
  tool_id: string;
  config_dir: string | null;
  mcp_config_path: string | null;
  skills_dir: string | null;
}

interface SettingsToolPathSectionTool {
  id: string;
  name: string;
  config_path: string;
  skills_dir: string;
  mcp_config_path: string;
  installed: boolean;
  install_command: string;
  install_url: string;
}

interface SettingsToolPathSectionProps {
  locale: Locale;
  visibleTools: SettingsToolPathSectionTool[];
  customPaths: SettingsToolPathSectionCustomPath[];
  pathSaved: string | null;
  onSaveMcpPath: (toolId: string, value: string, defaultValue: string, customPath?: SettingsToolPathSectionCustomPath) => void | Promise<void>;
  onPickMcpPath: (toolId: string, customPath?: SettingsToolPathSectionCustomPath) => void | Promise<void>;
  onSaveSkillsDir: (toolId: string, value: string, defaultValue: string, customPath?: SettingsToolPathSectionCustomPath) => void | Promise<void>;
  onPickSkillsDir: (toolId: string, customPath?: SettingsToolPathSectionCustomPath) => void | Promise<void>;
  onCopyInstallCommand: (command: string, toolName: string) => void | Promise<void>;
}

function SettingsToolPathSectionComponent({
  locale,
  visibleTools,
  customPaths,
  pathSaved,
  onSaveMcpPath,
  onPickMcpPath,
  onSaveSkillsDir,
  onPickSkillsDir,
  onCopyInstallCommand,
}: SettingsToolPathSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <FolderOpen size={17} style={{ color: "var(--text-secondary)" }} />
        {locale === "zh" ? "工具路径配置" : "Tool Path Configuration"}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        {visibleTools.map((tool) => {
          const custom = customPaths.find((item) => item.tool_id === tool.id);
          return (
            <SettingsToolPathCard
              key={tool.id}
              tool={tool}
              customPath={custom}
              locale={locale}
              saved={pathSaved === tool.id}
              onSaveMcpPath={onSaveMcpPath}
              onPickMcpPath={onPickMcpPath}
              onSaveSkillsDir={onSaveSkillsDir}
              onPickSkillsDir={onPickSkillsDir}
              onCopyInstallCommand={onCopyInstallCommand}
            />
          );
        })}
      </div>
    </div>
  );
}

export default memo(SettingsToolPathSectionComponent);
