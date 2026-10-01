import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ManagedAppId } from "../../lib/appPreferences";
import { fetchSessionsPageData, queryKeys } from "../../hooks/queries";
import {
  sameSession,
  sessionDetailArgs,
  sessionSelectionKey,
  type SessionDetail,
  type SessionSummary,
} from "./helpers";

export function useSessionData(filterTool: ManagedAppId | "all") {
  const queryClient = useQueryClient();
  const toolId = filterTool === "all" ? null : filterTool;
  const cached = queryClient.getQueryData<SessionSummary[]>(queryKeys.sessions(toolId));
  const [allSessions, setAllSessions] = useState<SessionSummary[]>(cached ?? []);
  const [loading, setLoading] = useState(!cached);
  const [refreshing, setRefreshing] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionSummary | null>(null);
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [detailQuery, setDetailQuery] = useState("");
  const listRequest = useRef(0);
  const detailRequest = useRef(0);
  const detailPending = useRef(false);
  const selected = useRef<SessionSummary | null>(null);
  const currentDetail = useRef<SessionDetail | null>(null);
  const scope = useRef(filterTool);
  scope.current = filterTool;

  const closeSession = useCallback(() => {
    detailRequest.current += 1;
    detailPending.current = false;
    selected.current = null;
    currentDetail.current = null;
    setSelectedSession(null);
    setDetail(null);
    setDetailLoading(false);
    setDetailError(null);
    setDetailQuery("");
  }, []);

  const removeSessions = useCallback(
    (removed: SessionSummary[]) => {
      const keys = new Set(removed.map(sessionSelectionKey));
      // A scan started before deletion must not restore a now-deleted record.
      listRequest.current += 1;
      void queryClient.cancelQueries({ queryKey: ["sessions"] });
      queryClient.setQueriesData<SessionSummary[]>({ queryKey: ["sessions"] }, (current) =>
        current?.filter((session) => !keys.has(sessionSelectionKey(session))),
      );
      setLoading(false);
      setRefreshing(false);
      if (selected.current && keys.has(sessionSelectionKey(selected.current))) closeSession();
      setAllSessions((current) => current.filter((session) => !keys.has(sessionSelectionKey(session))));
    },
    [closeSession, queryClient],
  );

  const openSession = useCallback(async (session: SessionSummary, updateSelection = true) => {
    if (scope.current !== "all" && scope.current !== session.tool_id) return;
    if (!updateSelection && !sameSession(selected.current, session)) return;
    const requestId = ++detailRequest.current;
    if (updateSelection) {
      selected.current = session;
      setSelectedSession(session);
    }
    const ownsRequest = () => requestId === detailRequest.current && sameSession(selected.current, session);
    detailPending.current = true;
    setDetailLoading(true);
    setDetailError(null);
    try {
      const next = await invoke<SessionDetail>("get_session_detail", sessionDetailArgs(session));
      if (!ownsRequest()) return;
      if (!sameSession(next.session, session)) throw new Error("Session detail response did not match its source");
      // A refresh can rename this exact session while its details are loading.
      // Retain the latest list metadata when the older response arrives.
      const merged = { ...next, session: { ...next.session, ...selected.current! } };
      currentDetail.current = merged;
      setDetail(merged);
      setDetailQuery("");
    } catch (error) {
      if (!ownsRequest()) return;
      currentDetail.current = null;
      setDetail(null);
      setDetailError(String(error));
    } finally {
      if (ownsRequest()) {
        detailPending.current = false;
        setDetailLoading(false);
      }
    }
  }, []);

  const loadSessions = useCallback(
    async (showLoading: boolean) => {
      const requestId = ++listRequest.current;
      const ownsRequest = () => requestId === listRequest.current && scope.current === filterTool;
      if (showLoading) setLoading(true);
      else setRefreshing(true);
      setLoadError(null);
      try {
        const next = await queryClient.fetchQuery({
          queryKey: queryKeys.sessions(toolId),
          queryFn: () => fetchSessionsPageData(toolId),
          staleTime: showLoading ? 30_000 : 0,
        });
        if (!ownsRequest()) return;
        setAllSessions(next);
        const previous = selected.current;
        if (previous) {
          const refreshed = next.find((item) => sameSession(item, previous));
          if (!refreshed) {
            closeSession();
            return;
          }
          selected.current = refreshed;
          setSelectedSession(refreshed);
          if (sameSession(currentDetail.current?.session, refreshed)) {
            const updated = { ...currentDetail.current!, session: { ...currentDetail.current!.session, ...refreshed } };
            currentDetail.current = updated;
            setDetail(updated);
          } else if (!detailPending.current) {
            void openSession(refreshed, false);
          }
        }
      } catch (error) {
        if (ownsRequest()) setLoadError(String(error));
        // Preserve the last successful list and detail on a failed refresh.
      } finally {
        if (ownsRequest()) {
          setLoading(false);
          setRefreshing(false);
        }
      }
    },
    [closeSession, filterTool, openSession, queryClient, toolId],
  );

  const loadRef = useRef(loadSessions);
  loadRef.current = loadSessions;
  const firstScope = useRef(true);
  useEffect(() => {
    const initial = firstScope.current;
    firstScope.current = false;
    if (!initial) {
      setAllSessions(queryClient.getQueryData<SessionSummary[]>(queryKeys.sessions(toolId)) ?? []);
      setLoadError(null);
      if (selected.current && filterTool !== "all" && selected.current.tool_id !== filterTool) closeSession();
    }
    const timer = window.setTimeout(() => void loadRef.current(initial), initial ? 0 : 180);
    return () => {
      window.clearTimeout(timer);
      listRequest.current += 1;
    };
  }, [closeSession, filterTool, queryClient, toolId]);

  useEffect(
    () => () => {
      listRequest.current += 1;
      detailRequest.current += 1;
    },
    [],
  );

  return {
    allSessions,
    loading,
    refreshing,
    loadError,
    selectedSession,
    detail,
    detailLoading,
    detailError,
    detailQuery,
    setDetailQuery,
    loadSessions,
    openSession,
    closeSession,
    removeSessions,
  };
}
