import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface DraftModelCheckResult {
  model: string;
  status: string;
  httpStatus: number | null;
  latencyMs: number | null;
  message: string;
}

export function useDraftModelChecks(scope: string, toolId: string, snapshot: string, selection: string) {
  const [results, setResults] = useState<DraftModelCheckResult[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const identity = JSON.stringify([scope, toolId, snapshot, selection]);
  const currentIdentity = useRef(identity);
  currentIdentity.current = identity;
  const request = useRef<object | null>(null);
  const mounted = useRef(false);

  useEffect(() => {
    mounted.current = true;
    request.current = null;
    setResults([]);
    setError(null);
    setRunning(false);
    return () => {
      mounted.current = false;
      request.current = null;
    };
  }, [identity]);

  const run = useCallback(
    async (models: string[]) => {
      if (request.current || !models.length) return;
      const token = {};
      request.current = token;
      const isCurrent = () => mounted.current && currentIdentity.current === identity && request.current === token;
      setRunning(true);
      setError(null);
      setResults([]);
      try {
        const next = await invoke<DraftModelCheckResult[]>("test_profile_draft", {
          toolId,
          configSnapshot: snapshot,
          models,
        });
        if (isCurrent()) setResults(next);
      } catch (reason) {
        if (isCurrent()) setError(String(reason));
      } finally {
        if (isCurrent()) {
          request.current = null;
          setRunning(false);
        }
      }
    },
    [identity, toolId, snapshot],
  );
  return { results, error, running, run };
}
