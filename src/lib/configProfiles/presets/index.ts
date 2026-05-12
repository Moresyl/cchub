import type { ConfigPreset, StructuredConfigTool } from "../types";
import { claudePresets } from "./claude";
import { codexPresets } from "./codex";
import { geminiPresets } from "./gemini";
import { openclawPresets } from "./openclaw";
import { hermesPresets } from "./hermes";
import { opencodePresets } from "./opencode";

export const PRESETS: Record<StructuredConfigTool, ConfigPreset[]> = {
  claude: claudePresets,
  codex: codexPresets,
  gemini: geminiPresets,
  openclaw: openclawPresets,
  hermes: hermesPresets,
  opencode: opencodePresets,
};
