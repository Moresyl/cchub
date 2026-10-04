import { useCallback, useEffect, useRef, useState } from "react";
import { showToast } from "../../components/Toast";
import type { McpServer } from "./helpers";

// This is the connection configuration available in the current catalog.
// Full native extensions are exported by the native catalog service migration.
function connectionConfig(server: McpServer) {
  const args: unknown = JSON.parse(server.args);
  const values: unknown = JSON.parse(server.env);
  if (
    !server.command ||
    !Array.isArray(args) ||
    args.some((value) => typeof value !== "string") ||
    !values ||
    typeof values !== "object" ||
    Array.isArray(values) ||
    Object.values(values).some((value) => typeof value !== "string")
  )
    throw new Error("Invalid connection configuration");
  if (server.transport === "stdio") return { command: server.command, args, env: values };
  if (server.transport !== "http" && server.transport !== "sse") throw new Error("Unsupported transport");
  const url = new URL(server.command);
  if (!["http:", "https:"].includes(url.protocol)) throw new Error("Invalid remote URL");
  return { type: server.transport, url: server.command, headers: values };
}

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
      const text = JSON.stringify(connectionConfig(server), null, 2);
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
