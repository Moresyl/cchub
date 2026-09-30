import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigProfile } from "../../pages/profiles/helpers";

export interface UsageResult {
  success?: boolean;
  provider?: string;
  data?: unknown;
  error?: string;
  asOf?: unknown;
  stale?: boolean;
}

interface QueryState {
  scope: string | null;
  result: UsageResult | null;
  error: string | null;
  loading: boolean;
  updatedAt: Date | null;
}

const empty: QueryState = { scope: null, result: null, error: null, loading: false, updatedAt: null };

export function useUsageResult(profile: ConfigProfile | null) {
  const id = profile?.id;
  const tool = profile?.tool_id;
  const scope = profile ? JSON.stringify([id, tool, profile.config_snapshot]) : null;
  const [state, setState] = useState<QueryState>(empty);
  const active = useRef({ generation: 0, busy: false });

  const refresh = useCallback(async () => {
    if (!scope || !id || !tool || active.current.busy) return;
    active.current.busy = true;
    const generation = ++active.current.generation;
    setState((previous) => ({ ...(previous.scope === scope ? previous : empty), scope, loading: true, error: null }));
    try {
      const result = await invoke<UsageResult>("queryProviderUsage", { providerId: id, app: tool });
      if (generation !== active.current.generation) return;
      const valid = result && typeof result === "object" && !Array.isArray(result);
      setState({
        scope,
        result: valid ? result : null,
        error: valid ? null : "Invalid usage response",
        loading: false,
        updatedAt: valid && result.success ? new Date() : null,
      });
    } catch (reason) {
      if (generation !== active.current.generation) return;
      setState((previous) => ({ ...previous, error: String(reason), loading: false }));
    } finally {
      if (generation === active.current.generation) active.current.busy = false;
    }
  }, [scope, id, tool]);

  useEffect(() => {
    const owner = active.current;
    void refresh();
    return () => {
      ++owner.generation;
      owner.busy = false;
    };
  }, [refresh]);

  const visible = state.scope === scope ? state : { ...empty, loading: scope !== null };
  return { ...visible, refresh };
}
