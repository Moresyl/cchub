import { Globe, Monitor, Sparkles, Terminal } from "lucide-react";
import type { SkillCardSkill } from "../../components/SkillCard";

export type Skill = SkillCardSkill;
export type { SkillCardSkill };

export interface Plugin {
  id: string;
  name: string;
  description: string | null;
  source_url: string | null;
  version: string | null;
}

export interface SkillBackup {
  id: string;
  skill_name: string;
  original_path: string;
  backup_path: string;
  created_at: string;
  size_bytes: number;
}

export const TOOL_ICONS: Record<string, typeof Monitor> = {
  claude: Terminal,
  codex: Monitor,
  gemini: Sparkles,
  opencode: Globe,
  hermes: Monitor,
  pi: Monitor,
};

export const PROMPT_PATTERN = /prompt|提示|template|模板|指令|instruction/i;

export function isPromptSkill(skill: Skill) {
  return (
    !skill.plugin_id && PROMPT_PATTERN.test(`${skill.name} ${skill.description || ""} ${skill.trigger_command || ""}`)
  );
}

export function isCommandSkill(skill: Skill) {
  return !skill.plugin_id && !isPromptSkill(skill) && !!skill.trigger_command;
}

export function isStandaloneSkill(skill: Skill) {
  return !skill.plugin_id && !isPromptSkill(skill) && !isCommandSkill(skill);
}

export function hasSkillUpdate(skill: Skill) {
  return Boolean(
    skill.source_url && skill.latest_sha256 && skill.current_sha256 && skill.latest_sha256 !== skill.current_sha256,
  );
}
