import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { RequestDetailRecord } from "../../components/RequestDetailPanel";

export function useRequestDetail(text: (zh: string, en: string) => string) {
  const [record, setRecord] = useState<RequestDetailRecord | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);
  const activeId = useRef<string | null>(null);
  useEffect(
    () => () => {
      generation.current += 1;
      activeId.current = null;
    },
    [],
  );

  const load = useCallback(
    async (requestId: string) => {
      const turn = ++generation.current;
      activeId.current = requestId;
      setRecord(null);
      setError(null);
      setLoading(true);
      try {
        const result = await invoke<RequestDetailRecord | null>("get_request_detail", { requestId });
        if (generation.current !== turn) return;
        setRecord(result);
        if (!result) setError(text("这条请求记录已被清理。", "This request record has been removed."));
      } catch {
        if (generation.current === turn)
          setError(text("请求明细加载失败，请重试。", "Could not load request details. Please retry."));
      } finally {
        if (generation.current === turn) setLoading(false);
      }
    },
    [text],
  );

  const close = useCallback(() => {
    generation.current += 1;
    activeId.current = null;
    setRecord(null);
    setError(null);
    setLoading(false);
  }, []);
  const retry = useCallback(() => {
    if (activeId.current) void load(activeId.current);
  }, [load]);
  return { record, loading, error, load, close, retry };
}
