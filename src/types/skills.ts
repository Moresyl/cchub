export interface DetectedTool {
  id: string;       // "claude" | "cursor" | "windsurf" | "codex" | "gemini" | "opencode"
  name: string;     // "Claude Code"
  config_path: string;
  skills_dir: string;
  mcp_config_path: string;
  installed: boolean;
  install_command: string;
  install_url: string;
}

export interface FolderNode {
  name: string;
  path: string;
  is_dir: boolean;
  children: FolderNode[];
}

export interface CategoryCounts {
  skills: number;
  prompts: number;
  commands: number;
  plugins: number;
  total: number;
}

export type SkillCategory = "all" | "skill" | "prompt" | "command" | "plugin";
