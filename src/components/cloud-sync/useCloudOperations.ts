import { useCallback, useEffect, useLayoutEffect, useMemo, useRef } from "react";

export interface CloudOperation {
  current: () => boolean;
  finish: () => void;
}

// A write owns its subsequent reads. Background reads never take over an
// action, and editing immediately invalidates a pending background response.
export function useCloudOperations() {
  const active = useRef({ generation: 0, mounted: false, mode: "idle" as "idle" | "read" | "action" });
  useEffect(() => {
    const owner = active.current;
    owner.mounted = true;
    return () => {
      owner.mounted = false;
      owner.mode = "idle";
      ++owner.generation;
    };
  }, []);

  const begin = useCallback((mode: "read" | "action"): CloudOperation | null => {
    const owner = active.current;
    if (!owner.mounted || owner.mode === "action" || (mode === "read" && owner.mode === "read")) return null;
    const generation = ++owner.generation;
    owner.mode = mode;
    const current = () => owner.mounted && owner.generation === generation;
    return {
      current,
      finish: () => {
        if (current()) {
          owner.mode = "idle";
          ++owner.generation;
        }
      },
    };
  }, []);
  const beginRead = useCallback(() => begin("read"), [begin]);
  const beginAction = useCallback(() => begin("action"), [begin]);
  const invalidateReads = useCallback(() => {
    const owner = active.current;
    if (owner.mode === "read") {
      owner.mode = "idle";
      ++owner.generation;
    }
  }, []);
  return useMemo(() => ({ beginRead, beginAction, invalidateReads }), [beginRead, beginAction, invalidateReads]);
}

export function useCloudCallback<Args extends unknown[]>(callback: (...args: Args) => void) {
  const latest = useRef<((...args: Args) => void) | null>(callback);
  useLayoutEffect(() => {
    latest.current = callback;
    return () => {
      latest.current = null;
    };
  }, [callback]);
  return useCallback((...args: Args) => latest.current?.(...args), []);
}

export function useCloudWindowRefresh(refresh: () => void) {
  const onRefresh = useCloudCallback(refresh);
  useEffect(() => {
    const visibleRefresh = () => {
      if (document.visibilityState === "visible") onRefresh();
    };
    window.addEventListener("focus", visibleRefresh);
    document.addEventListener("visibilitychange", visibleRefresh);
    return () => {
      window.removeEventListener("focus", visibleRefresh);
      document.removeEventListener("visibilitychange", visibleRefresh);
    };
  }, [onRefresh]);
}
