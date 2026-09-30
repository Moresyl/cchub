import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AlertOverview } from "./types";

export function useUsageAlerts() {
  const [data, setData] = useState<AlertOverview | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const active = useRef({ generation: 0, mounted: true, busy: false });

  const request = useCallback(async (command: string, args?: Record<string, unknown>, writing = false) => {
    const owner = active.current;
    const generation = ++owner.generation;
    try {
      const next = await invoke<AlertOverview>(command, args);
      if (!next || !Array.isArray(next.rules) || !Array.isArray(next.events))
        throw new Error("Invalid usage alert response");
      if (owner.mounted && generation === owner.generation) {
        setData(next);
        setLoadError(null);
      }
      return true;
    } catch (reason) {
      if (owner.mounted) {
        if (writing) setActionError(String(reason));
        else if (generation === owner.generation) setLoadError(String(reason));
      }
      return false;
    }
  }, []);

  const refresh = useCallback(() => {
    if (!active.current.busy) setActionError(null);
    return request("get_usage_alerts");
  }, [request]);
  const action = useCallback(
    async (command: string, args?: Record<string, unknown>) => {
      const owner = active.current;
      if (owner.busy) return false;
      owner.busy = true;
      setBusy(true);
      setActionError(null);
      try {
        return await request(command, args, true);
      } finally {
        owner.busy = false;
        if (owner.mounted) setBusy(false);
      }
    },
    [request],
  );

  useEffect(() => {
    const owner = active.current;
    owner.mounted = true;
    void request("get_usage_alerts");
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("usage-alerts-changed", () => {
      void request("get_usage_alerts");
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {
        /* Manual refresh remains available when events are unavailable. */
      });
    return () => {
      disposed = true;
      owner.mounted = false;
      ++owner.generation;
      unlisten?.();
    };
  }, [request]);

  return { data, error: actionError ?? loadError, busy, refresh, action };
}
