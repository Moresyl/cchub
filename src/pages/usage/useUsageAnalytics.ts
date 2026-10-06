import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UsageAnalytics, UsageFilters } from "./types";

interface Snapshot {
  key: string;
  data: UsageAnalytics | null;
  loading: boolean;
  error: boolean;
}

const LIVE_REFRESH_DELAY = 250;

export function useUsageAnalytics({ days, appId, providerName, model }: UsageFilters) {
  const key = JSON.stringify([days, appId, providerName, model]);
  const catalogKey = JSON.stringify([days, appId]);
  const [snapshot, setSnapshot] = useState<Snapshot>({ key, data: null, loading: true, error: false });
  const [catalog, setCatalog] = useState({ key: catalogKey, providers: [] as string[], models: [] as string[] });
  const [liveUnavailable, setLiveUnavailable] = useState(false);
  const refreshRef = useRef<(() => void) | null>(null);
  const refresh = useCallback(() => refreshRef.current?.(), []);

  useEffect(() => {
    let active = true;
    let pending = false;
    let dirty = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let unlisten: (() => void) | undefined;
    const schedule = () => {
      if (!active) return;
      if (pending) {
        dirty = true;
      } else if (timer === undefined) {
        timer = setTimeout(() => {
          timer = undefined;
          void load();
        }, LIVE_REFRESH_DELAY);
      }
    };
    const load = async () => {
      if (!active) return;
      if (pending) {
        dirty = true;
        return;
      }
      clearTimeout(timer);
      timer = undefined;
      pending = true;
      setSnapshot((current) => ({ key, data: current.key === key ? current.data : null, loading: true, error: false }));
      try {
        const data = await invoke<UsageAnalytics>("get_usage_analytics", {
          days,
          appId: appId || null,
          providerName: providerName || null,
          model: model || null,
        });
        if (!active) return;
        setSnapshot({ key, data, loading: false, error: false });
        // Keep options from the broader result while a provider or model is selected.
        setCatalog((current) => ({
          key: catalogKey,
          providers: [
            ...new Set([
              ...(current.key === catalogKey && (providerName || model) ? current.providers : []),
              ...data.providers.map((row) => row.provider_name),
            ]),
          ],
          models: [
            ...new Set([
              ...(current.key === catalogKey && (providerName || model) ? current.models : []),
              ...data.models.map((row) => row.model),
            ]),
          ],
        }));
      } catch {
        if (active) setSnapshot((current) => ({ ...current, loading: false, error: true }));
      } finally {
        pending = false;
        if (active && dirty) {
          dirty = false;
          schedule();
        }
      }
    };
    const manualRefresh = () => void load();
    refreshRef.current = manualRefresh;
    setLiveUnavailable(false);
    void load();
    void listen("usage-log-recorded", schedule).then(
      (dispose) => {
        if (active) unlisten = dispose;
        else dispose();
      },
      () => {
        if (active) setLiveUnavailable(true);
      },
    );
    return () => {
      active = false;
      clearTimeout(timer);
      unlisten?.();
      if (refreshRef.current === manualRefresh) refreshRef.current = null;
    };
  }, [days, appId, providerName, model, key, catalogKey]);

  const current = snapshot.key === key ? snapshot : { data: null, loading: true, error: false };
  return {
    ...current,
    refresh,
    liveUnavailable,
    providers: catalog.key === catalogKey ? catalog.providers : [],
    models: catalog.key === catalogKey ? catalog.models : [],
  };
}
