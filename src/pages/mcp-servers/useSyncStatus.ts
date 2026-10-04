import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../../components/Toast";
import type { McpServer } from "./helpers";
import { knownMcpStates, readMcpStatuses, type McpToolStatuses } from "../../lib/mcpCatalog";

type Status = Record<string, boolean>;
interface Snapshot {
  id: string;
  status: Status;
  states: McpToolStatuses;
  loading: boolean;
  error: boolean;
}

export function useSyncStatus(selected: McpServer | null, zh: boolean) {
  const current = useRef(selected);
  current.current = selected;
  const generation = useRef(0);
  const operation = useRef(false);
  const [syncingTo, setSyncingTo] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const snapshotRef = useRef(snapshot);
  snapshotRef.current = snapshot;
  const read = useCallback(async () => {
    const server = current.current;
    if (!server) return;
    const request = ++generation.current;
    snapshotRef.current = null;
    setSnapshot({ id: server.id, status: {}, states: {}, loading: true, error: false });
    try {
      const states = readMcpStatuses(await invoke("get_mcp_sync_statuses", { serverIds: [server.id] }), [server.id])[
        server.id
      ];
      const status = knownMcpStates(states);
      if (request === generation.current && current.current?.id === server.id) {
        setSnapshot({ id: server.id, status, states, loading: false, error: false });
      }
    } catch {
      if (request === generation.current && current.current?.id === server.id) {
        setSnapshot({ id: server.id, status: {}, states: {}, loading: false, error: true });
      }
    }
  }, []);
  useEffect(() => {
    const requests = generation;
    void read();
    return () => {
      ++requests.current;
    };
  }, [selected?.id, selected?.name, read]);

  const toggle = useCallback(
    async (toolId: string) => {
      const server = current.current;
      const known = snapshotRef.current;
      if (
        !server ||
        operation.current ||
        known?.id !== server.id ||
        known.loading ||
        known.error ||
        typeof known.status[toolId] !== "boolean"
      )
        return null;
      operation.current = true;
      setSyncingTo(toolId);
      ++generation.current;
      const enabled = !known.status[toolId];
      try {
        await invoke(enabled ? "sync_mcp_server_to_tool" : "unsync_mcp_server_from_tool", {
          serverName: server.id,
          targetTool: toolId,
        });
        if (current.current?.id === server.id) await read();
        return { serverId: server.id, toolId, enabled };
      } catch {
        showToast("error", zh ? "同步操作失败，请刷新状态后重试。" : "Sync failed. Refresh the status and retry.");
        if (current.current?.id === server.id) await read();
        return null;
      } finally {
        operation.current = false;
        setSyncingTo(null);
      }
    },
    [read, zh],
  );
  const matches = selected && snapshot?.id === selected.id;
  return {
    status: matches ? snapshot.status : {},
    states: matches ? snapshot.states : {},
    loading: !!selected && (!matches || snapshot.loading),
    error: !!matches && snapshot.error,
    syncingTo,
    toggle,
    refresh: read,
  };
}
