import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface EndpointLatency {
  url: string;
  latency: number | null;
  status: number | null;
  error: string | null;
}

export function normalizeEndpoint(value: string): string {
  return value.trim().replace(/\/+$/, "");
}

export function collectProbeEndpoints(baseUrl: string, candidates: string, customEndpoints: string[]): string[] {
  const valid = new Set<string>();
  for (const raw of [baseUrl, ...candidates.split(/[,\n]/), ...customEndpoints]) {
    const value = normalizeEndpoint(raw);
    try {
      const parsed = new URL(value);
      if (["http:", "https:"].includes(parsed.protocol) && !parsed.username && !parsed.password) valid.add(value);
    } catch {
      // Incomplete editor values remain in the draft, but cannot be probed yet.
    }
  }
  return [...valid].slice(0, 64);
}

export function useEndpointProbe(scope: string, entries: string[]) {
  const [results, setResults] = useState<EndpointLatency[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const identity = JSON.stringify([scope, entries]);
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

  const run = useCallback(async () => {
    if (!entries.length || request.current) return;
    const token = {};
    request.current = token;
    const isCurrent = () => mounted.current && currentIdentity.current === identity && request.current === token;
    setRunning(true);
    setError(null);
    setResults([]);
    try {
      const next = await invoke<EndpointLatency[]>("test_api_endpoints", { urls: entries, timeoutSecs: 10 });
      if (isCurrent()) {
        setResults([...next].sort((a, b) => (a.latency ?? Infinity) - (b.latency ?? Infinity)));
      }
    } catch (reason) {
      if (isCurrent()) setError(String(reason));
    } finally {
      if (isCurrent()) {
        request.current = null;
        setRunning(false);
      }
    }
  }, [identity, entries]);

  return { results, running, error, run };
}
