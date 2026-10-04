import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../../components/Toast";
import type { McpServer } from "./helpers";

export function useConfigCopy(selected: McpServer | null, zh: boolean) {
  const current = useRef(selected);
  current.current = selected;
  const pending = useRef(false);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    setCopiedId(null);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [selected?.id]);
  const copy = useCallback(async () => {
    const server = current.current;
    if (!server || pending.current) return;
    pending.current = true;
    setCopying(true);
    setCopiedId(null);
    if (timer.current) clearTimeout(timer.current);
    try {
      const text = await invoke<string>("export_mcp_server_config", { serverId: server.id });
      const config: unknown = JSON.parse(text);
      if (!config || typeof config !== "object" || Array.isArray(config))
        throw new Error("Invalid exported configuration");
      await navigator.clipboard.writeText(text);
      if (current.current?.id === server.id) {
        setCopiedId(server.id);
        timer.current = setTimeout(() => setCopiedId(null), 2000);
      }
    } catch {
      showToast(
        "error",
        zh
          ? "未能复制配置，请检查配置内容和剪贴板权限后重试。"
          : "Could not copy configuration. Check the configuration and clipboard permissions, then retry.",
      );
    } finally {
      pending.current = false;
      setCopying(false);
    }
  }, [zh]);
  return { copy, copying, copied: !!selected && copiedId === selected.id };
}
