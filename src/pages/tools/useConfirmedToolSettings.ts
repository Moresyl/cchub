import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

interface Configuration<T extends { config_revision: string }> {
  queryKey: readonly string[];
  read: () => Promise<T>;
  write: (key: string, value: string, revision: string) => Promise<T>;
}

export function useConfirmedToolSettings<T extends { config_revision: string }>(config: Configuration<T>) {
  const client = useQueryClient();
  const generation = useRef(0);
  const mounted = useRef(true);
  const pending = useRef(false);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current += 1;
    };
  }, []);
  const query = useQuery({
    queryKey: config.queryKey,
    queryFn: async () => {
      generation.current += 1;
      return config.read();
    },
    staleTime: 0,
    retry: false,
    throwOnError: false,
    refetchOnMount: "always",
    networkMode: "always",
  });
  const { refetch } = query;
  const reload = useCallback(async () => {
    if (pending.current) return;
    setSaveError(false);
    await refetch();
  }, [refetch]);
  const save = useCallback(
    async (key: string, value: string) => {
      const current = client.getQueryData<T>(config.queryKey);
      if (pending.current || query.isFetching || query.isError || !current?.config_revision || saveError) return false;
      pending.current = true;
      setSaving(true);
      setSaveError(false);
      try {
        await client.cancelQueries({ queryKey: config.queryKey });
        const attempt = ++generation.current;
        const result = await config.write(key, value, current.config_revision);
        if (mounted.current && generation.current === attempt) client.setQueryData(config.queryKey, result);
        else void client.invalidateQueries({ queryKey: config.queryKey });
        return true;
      } catch {
        if (mounted.current) setSaveError(true);
        return false;
      } finally {
        pending.current = false;
        if (mounted.current) setSaving(false);
      }
    },
    [client, config, query.isError, query.isFetching, saveError],
  );
  return {
    settings: query.data,
    loading: query.isPending,
    refreshing: query.isFetching,
    loadError: query.isError,
    saveError,
    saving,
    save,
    reload,
  };
}
