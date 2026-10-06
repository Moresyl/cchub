import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { parseSnapshot, type LibrarySnapshot, type PromptApp } from "./types";
import { READBACK_ERROR } from "./errors";

export { READBACK_ERROR };

export function usePromptLibrary(app: PromptApp) {
  const [snapshot, setSnapshot] = useState<{ app: PromptApp; value: LibrarySnapshot } | null>(null);
  const [loading, setLoading] = useState(true);
  const [writing, setWriting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needsReload, setNeedsReload] = useState(false);
  const owner = useRef(app);
  owner.current = app;
  const generation = useRef(0);
  const request = useRef(0);
  const mounted = useRef(true);
  const busy = useRef(false);
  const blocked = useRef(false);
  const reading = useRef(0);
  const snapshotRef = useRef(snapshot);
  snapshotRef.current = snapshot;
  const invalidate = useCallback(() => {
    generation.current++;
    request.current++;
  }, []);

  const load = useCallback(async (): Promise<LibrarySnapshot | null> => {
    const version = generation.current;
    const sequence = ++request.current;
    reading.current = sequence;
    const owns = () =>
      mounted.current && owner.current === app && generation.current === version && request.current === sequence;
    setLoading(true);
    try {
      const value = parseSnapshot(await invoke<LibrarySnapshot>("get_prompt_library_snapshot", { app }));
      if (!owns()) return null;
      setSnapshot({ app, value });
      snapshotRef.current = { app, value };
      setError(null);
      setNeedsReload(false);
      blocked.current = false;
      return value;
    } catch (failure) {
      if (owns()) {
        setError(String(failure));
        setNeedsReload(true);
        blocked.current = true;
      }
      return null;
    } finally {
      if (owns()) {
        setLoading(false);
        reading.current = 0;
      }
    }
  }, [app]);

  useEffect(() => {
    mounted.current = true;
    invalidate();
    setSnapshot(null);
    snapshotRef.current = null;
    setError(null);
    setNeedsReload(false);
    blocked.current = false;
    setWriting(busy.current);
    void load();
    return () => {
      mounted.current = false;
      invalidate();
    };
  }, [load, invalidate]);

  const refresh = useCallback(() => (busy.current ? Promise.resolve(null) : load()), [load]);
  const mutate = useCallback(
    async (command: string, args: Record<string, unknown> = {}) => {
      const current = snapshotRef.current;
      if (busy.current || blocked.current || reading.current || current?.app !== app) return false;
      const version = generation.current;
      const owns = () => mounted.current && owner.current === app && generation.current === version;
      busy.current = true;
      setWriting(true);
      setError(null);
      ++request.current;
      try {
        await invoke(command, {
          expectedLibraryRevision: current.value.libraryRevision,
          expectedLiveRevision: current.value.live?.revision,
          ...args,
          app,
        });
        if (!owns()) return false;
        const updated = await load();
        if (!updated && owns()) {
          setError(READBACK_ERROR);
          setNeedsReload(true);
          blocked.current = true;
        }
        return updated !== null && owns();
      } catch (failure) {
        if (owns()) {
          setError(String(failure));
          setNeedsReload(true);
          blocked.current = true;
        }
        return false;
      } finally {
        busy.current = false;
        if (mounted.current) setWriting(false);
      }
    },
    [app, load],
  );
  return {
    snapshot: snapshot?.app === app ? snapshot.value : null,
    loading,
    writing,
    error,
    needsReload,
    refresh,
    mutate,
    isWriting: () => busy.current,
  };
}
