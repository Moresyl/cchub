import { useRef } from "react";
import type { SettingsToolPathCardCustomPath } from "../components/SettingsToolPathCard";

type Paths = SettingsToolPathCardCustomPath[];
type Field = "mcp_config_path" | "skills_dir";
interface Options {
  customPaths: Paths;
  setCustomPaths: (update: (paths: Paths) => Paths) => void;
  save: (toolId: string, configDir: string | null, mcpPath: string | null, skillsDir: string | null) => Promise<void>;
  pickFile: () => Promise<string | null>;
  pickFolder: () => Promise<string | null>;
}

export function useSettingsToolPathActions(options: Options) {
  const current = useRef(options);
  current.current = options;

  const saveField = async (toolId: string, field: Field, override: string | null) => {
    const latest = current.current;
    const path = latest.customPaths.find((item) => item.tool_id === toolId);
    if (override === (path?.[field] || null)) return;
    const updated = {
      tool_id: toolId,
      config_dir: path?.config_dir || null,
      mcp_config_path: path?.mcp_config_path || null,
      skills_dir: path?.skills_dir || null,
      [field]: override,
    };
    await latest.save(toolId, updated.config_dir, updated.mcp_config_path, updated.skills_dir);
    // The write is committed even if the subsequent detection refresh failed.
    // Retain its known result locally so a later edit cannot restore old paths.
    current.current.setCustomPaths((paths) => [...paths.filter((item) => item.tool_id !== toolId), updated]);
  };
  const saveDraft = async (toolId: string, field: Field, value: string, defaultValue: string) => {
    const trimmed = value.trim();
    const override = trimmed && trimmed !== defaultValue ? trimmed : null;
    await saveField(toolId, field, override);
    return override || defaultValue;
  };
  const pick = async (toolId: string, field: Field) => {
    const picked = await (field === "mcp_config_path" ? current.current.pickFile() : current.current.pickFolder());
    if (!picked) return null;
    // Resolve the other fields after the chooser returns, rather than restoring
    // a snapshot from when the chooser was opened.
    await saveField(toolId, field, picked);
    return picked;
  };

  return {
    onSaveMcpPath: (toolId: string, value: string, defaultValue: string) =>
      saveDraft(toolId, "mcp_config_path", value, defaultValue),
    onSaveSkillsDir: (toolId: string, value: string, defaultValue: string) =>
      saveDraft(toolId, "skills_dir", value, defaultValue),
    onPickMcpPath: (toolId: string) => pick(toolId, "mcp_config_path"),
    onPickSkillsDir: (toolId: string) => pick(toolId, "skills_dir"),
  };
}
