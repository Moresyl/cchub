import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { normalizeModelCatalog, type ModelInfo } from "../../lib/modelCatalog";

export interface CliQuota {
  credentialStatus: string;
  success: boolean;
  tiers: { name: string; utilization: number; resetsAt?: string | null }[];
}
export interface Resource<T> {
  status: "loading" | "ready" | "error";
  data?: T;
}
interface State {
  codex: Resource<CliQuota>;
  claude: Resource<CliQuota>;
  models: Resource<ModelInfo[]>;
}
function quota(value: unknown): CliQuota {
  if (!value || typeof value !== "object") throw new Error("Invalid quota");
  const row = value as CliQuota;
  if (typeof row.credentialStatus !== "string" || typeof row.success !== "boolean" || !Array.isArray(row.tiers))
    throw new Error("Invalid quota");
  return {
    credentialStatus: row.credentialStatus,
    success: row.success,
    tiers: row.tiers
      .filter((tier) => tier && typeof tier.name === "string" && Number.isFinite(tier.utilization))
      .map((tier) => ({
        name: tier.name,
        utilization: Math.max(0, Math.min(100, tier.utilization)),
        resetsAt: typeof tier.resetsAt === "string" ? tier.resetsAt : null,
      })),
  };
}
export function useCliUsage() {
  const [state, setState] = useState<State>({
    codex: { status: "loading" },
    claude: { status: "loading" },
    models: { status: "loading" },
  });
  const active = useRef({ generation: 0, busy: false });
  const refresh = useCallback(async () => {
    if (active.current.busy) return;
    active.current.busy = true;
    const generation = ++active.current.generation;
    setState((previous) => ({
      codex: { ...previous.codex, status: "loading" },
      claude: { ...previous.claude, status: "loading" },
      models: { ...previous.models, status: "loading" },
    }));
    async function read<K extends keyof State>(key: K, command: string, decode: (value: unknown) => State[K]["data"]) {
      try {
        const data = decode(await invoke<unknown>(command));
        if (active.current.generation === generation)
          setState((previous) => ({ ...previous, [key]: { status: "ready", data } }));
      } catch {
        if (active.current.generation === generation)
          setState((previous) => ({ ...previous, [key]: { ...previous[key], status: "error" } }));
      }
    }
    await Promise.all([
      read("codex", "get_codex_cli_quota", quota),
      read("claude", "get_claude_cli_quota", quota),
      read("models", "get_codex_cli_models", (value) => {
        const catalog = normalizeModelCatalog({ toolId: "codex", models: value });
        if (!catalog) throw new Error("Invalid catalog");
        return catalog.models;
      }),
    ]);
    if (active.current.generation === generation) active.current.busy = false;
  }, []);
  useEffect(() => {
    const owner = active.current;
    void refresh();
    return () => {
      ++owner.generation;
      owner.busy = false;
    };
  }, [refresh]);
  return { ...state, loading: Object.values(state).some((resource) => resource.status === "loading"), refresh };
}
