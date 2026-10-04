import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { fetchMcpServersPageData, queryKeys } from "../../hooks/queries";
import type { McpServer } from "./helpers";
import { knownMcpStates, readMcpStatuses } from "../../lib/mcpCatalog";

type PageData = Awaited<ReturnType<typeof fetchMcpServersPageData>>;
type AppStatus = Record<string, Record<string, boolean>>;

export function usePageData(zh: boolean) {
  const queryClient = useQueryClient();
  const cached = queryClient.getQueryData<PageData>(queryKeys.mcpServersPage);
  const [servers, setServers] = useState<McpServer[]>(cached?.servers ?? []);
  const [tools, setTools] = useState(cached?.tools ?? []);
  const [selected, setSelected] = useState<McpServer | null>(null);
  const [loading, setLoading] = useState(!cached);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [serverAppStatus, setServerAppStatus] = useState<AppStatus>({});
  const [appStatusLoading, setAppStatusLoading] = useState(false);
  const generation = useRef(0);

  const loadPageData = useCallback(
    async (options: { force?: boolean } = {}) => {
      const request = ++generation.current;
      const ownsRequest = () => request === generation.current;
      if (!queryClient.getQueryData(queryKeys.mcpServersPage)) setLoading(true);
      setLoadError(null);
      // Old known states must not enable bulk writes while their source is refreshed.
      setAppStatusLoading(true);
      setServerAppStatus({});
      try {
        const data = await queryClient.fetchQuery({
          queryKey: queryKeys.mcpServersPage,
          queryFn: fetchMcpServersPageData,
          staleTime: options.force ? 0 : 30_000,
        });
        if (!ownsRequest()) return;
        setServers(data.servers);
        setTools(data.tools);
        setSelected((current) => (current ? (data.servers.find((server) => server.id === current.id) ?? null) : null));
        setLoading(false);
        const ids = data.servers.filter((server) => server.origin).map((server) => server.id);
        const results = ids.length
          ? readMcpStatuses(await invoke("get_mcp_sync_statuses", { serverIds: ids }), ids)
          : {};
        if (!ownsRequest()) return;
        const next: AppStatus = {};
        for (const id of ids) {
          next[id] = knownMcpStates(results[id]);
        }
        setServerAppStatus(next);
      } catch {
        if (ownsRequest()) setLoadError(zh ? "无法刷新服务列表，请重试。" : "Could not refresh servers. Please retry.");
      } finally {
        if (ownsRequest()) {
          setLoading(false);
          setAppStatusLoading(false);
        }
      }
    },
    [queryClient, zh],
  );

  useEffect(() => {
    void loadPageData();
    const requests = generation;
    return () => {
      ++requests.current;
    };
  }, [loadPageData]);

  return {
    servers,
    setServers,
    tools,
    selected,
    setSelected,
    loading,
    loadError,
    serverAppStatus,
    setServerAppStatus,
    appStatusLoading,
    loadPageData,
  };
}
