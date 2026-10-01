export const DEFAULT_CODEX_REASONING_LEVELS = ["low", "medium", "high", "xhigh"];

/** An empty reported list means no levels; an absent list means unknown. */
export function codexReasoningChoices(value: string, reportedLevels?: string[] | null) {
  const reported = Array.isArray(reportedLevels);
  const levels = reported
    ? [...new Set(reportedLevels.filter((level) => level.trim()).map((level) => level.trim()))]
    : DEFAULT_CODEX_REASONING_LEVELS;
  return { reported, levels, unsupported: reported && value !== "" && !levels.includes(value) };
}
