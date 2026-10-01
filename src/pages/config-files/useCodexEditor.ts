import { useMemo, useState } from "react";
import { ParseError } from "toml-eslint-parser";
import {
  normalizeCodexInteger,
  parseCodexStructuredConfig,
  repairCodexConfigContent,
  updateCodexStructuredContent,
  validateCodexStructuredConfig,
  type CodexStructuredConfig,
} from "../../lib/codexConfig";

type Draft = Partial<CodexStructuredConfig>;

export function useCodexEditor(content: string, onChange: (content: string) => void, scope: string, active: boolean) {
  const [pending, setPending] = useState<{ content: string; scope: string; patch: Draft } | null>(null);
  const parsed = useMemo(() => {
    if (!active) return { config: null, error: null };
    try {
      return { config: parseCodexStructuredConfig(content), error: null };
    } catch (error) {
      return {
        config: null,
        error:
          error instanceof ParseError
            ? `TOML (${error.lineNumber}:${error.column + 1}): ${error.message}`
            : String(error instanceof Error ? error.message : error),
      };
    }
  }, [content, active]);
  const drafts = pending?.content === content && pending.scope === scope ? pending.patch : {};
  const config = parsed.config ? { ...parsed.config, ...drafts } : null;
  const validation = config ? validateCodexStructuredConfig(config) : null;
  const reset = () => setPending(null);
  function update(patch: Draft) {
    if (!parsed.config) throw new Error("Correct the raw TOML before editing structured fields");
    const applied: Draft = {};
    const nextDraft = { ...drafts };
    for (const [field, value] of Object.entries(patch)) {
      const key = field as keyof CodexStructuredConfig;
      if (
        (key === "modelContextWindow" || key === "modelAutoCompactTokenLimit") &&
        normalizeCodexInteger(String(value)) === null
      ) {
        nextDraft[key] = String(value);
      } else {
        Object.assign(applied, { [key]: value });
        delete nextDraft[key];
      }
    }
    const next = updateCodexStructuredContent(content, applied);
    setPending({ content: next, scope, patch: nextDraft });
    onChange(next);
  }
  function addMcpTable() {
    const next = repairCodexConfigContent(content);
    setPending({ content: next, scope, patch: drafts });
    onChange(next);
  }
  return {
    config,
    validation,
    error: parsed.error,
    invalidContextWindow: config ? normalizeCodexInteger(config.modelContextWindow) === null : false,
    invalidCompactLimit: config ? normalizeCodexInteger(config.modelAutoCompactTokenLimit) === null : false,
    hasPendingDraft: active && Object.keys(drafts).length > 0,
    update,
    addMcpTable,
    reset,
  };
}
