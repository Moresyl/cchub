import type { Hello2ccConfigQueryResult } from "../../hooks/queries";

export type ToolTab = "claude" | "codex";

export interface HudStatus {
  installed: boolean;
  version: string;
  indexJsPath: string;
  statuslineEnabled: boolean;
  hudConfig: HudConfig;
}

export interface HudConfig {
  lineLayout?: "compact" | "expanded";
  showSeparators?: boolean;
  pathLevels?: number;
  elementOrder?: string[];
  gitStatus?: {
    enabled?: boolean;
    showDirty?: boolean;
    showAheadBehind?: boolean;
    showFileStats?: boolean;
  };
  display?: {
    showModel?: boolean;
    showProject?: boolean;
    showContextBar?: boolean;
    contextValue?: "percent" | "tokens" | "remaining" | "both";
    showConfigCounts?: boolean;
    showDuration?: boolean;
    showSpeed?: boolean;
    showUsage?: boolean;
    usageBarEnabled?: boolean;
    showTokenBreakdown?: boolean;
    showTools?: boolean;
    showAgents?: boolean;
    showTodos?: boolean;
    showSessionName?: boolean;
    showClaudeCodeVersion?: boolean;
    showMemoryUsage?: boolean;
  };
}

export type Hello2ccConfig = Hello2ccConfigQueryResult;
export type Hello2ccSelectKey = Exclude<keyof Hello2ccConfig, "mirror_session_model">;
export type HudGitStatusKey = keyof NonNullable<HudConfig["gitStatus"]>;
export type HudDisplayBooleanKey = Exclude<keyof NonNullable<HudConfig["display"]>, "contextValue">;

export interface Hello2ccUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
}

export const DEFAULT_HUD_CONFIG: HudConfig = {
  lineLayout: "expanded",
  showSeparators: false,
  pathLevels: 1,
  gitStatus: { enabled: true, showDirty: true, showAheadBehind: false, showFileStats: false },
  display: {
    showModel: true,
    showProject: true,
    showContextBar: true,
    contextValue: "percent",
    showConfigCounts: false,
    showDuration: false,
    showSpeed: false,
    showUsage: true,
    usageBarEnabled: true,
    showTokenBreakdown: true,
    showTools: false,
    showAgents: false,
    showTodos: false,
    showSessionName: false,
    showClaudeCodeVersion: false,
    showMemoryUsage: false,
  },
};

export const DEFAULT_HELLO2CC_CONFIG: Hello2ccConfig = {
  routing_policy: "native-inject",
  mirror_session_model: true,
  default_agent_model: "",
  primary_model: "",
  subagent_model: "",
  guide_model: "",
  explore_model: "",
  plan_model: "",
  general_model: "",
  team_model: "",
  compatibility_mode: "full",
};

export const PERM_LEVELS = [
  { label_zh: "严格", label_en: "Strict", label_ja: "厳格", color: "#ef4444" },
  { label_zh: "标准", label_en: "Standard", label_ja: "標準", color: "#eab308" },
  { label_zh: "宽松", label_en: "Relaxed", label_ja: "緩和", color: "#3b82f6" },
  { label_zh: "全部允许", label_en: "Allow All", label_ja: "すべて許可", color: "#22c55e" },
];

export const PERM_DESC_ZH = ["每次操作都确认", "允许读取，写操作确认", "允许读写，仅 Bash 确认", "跳过所有确认"];
export const PERM_DESC_EN = [
  "Confirm every action",
  "Allow read, confirm write",
  "Allow read/write, confirm Bash",
  "Skip all prompts",
];
export const PERM_DESC_JA = [
  "毎回すべて確認",
  "読み取りを許可し、書き込みは確認",
  "読み書きを許可し、Bash のみ確認",
  "すべての確認をスキップ",
];
