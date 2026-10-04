import { useCallback, useRef, useState } from "react";
import { showToast } from "../../components/Toast";

interface Draft {
  name: string;
  transport: string;
  command: string;
  args: string;
  env: string;
}

interface Config {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
}

function parse(draft: Draft): Config {
  const remote = draft.transport === "http" || draft.transport === "sse";
  if (!remote && draft.transport !== "stdio") throw new Error("Unsupported transport");
  const command = draft.command.trim();
  if (remote && !["http:", "https:"].includes(new URL(command).protocol)) throw new Error("Invalid remote URL");
  const args: unknown = remote ? [] : JSON.parse(draft.args);
  const env: unknown = JSON.parse(draft.env);
  if (
    !draft.command.trim() ||
    draft.command.includes("\0") ||
    !Array.isArray(args) ||
    args.some((value) => typeof value !== "string" || value.includes("\0")) ||
    !env ||
    typeof env !== "object" ||
    Array.isArray(env) ||
    Object.entries(env).some(
      ([key, value]) => !key || key.includes("\0") || typeof value !== "string" || value.includes("\0"),
    )
  )
    throw new Error("Invalid configuration fields");
  return { name: draft.name, command, args, env: env as Record<string, string> };
}

export function useConfigSave(zh: boolean) {
  const locked = useRef(false);
  const [saving, setSaving] = useState(false);
  const save = useCallback(
    async (draft: Draft, write: (config: Config) => Promise<unknown>) => {
      if (locked.current) return false;
      let config: Config;
      try {
        config = parse(draft);
      } catch {
        const remote = draft.transport === "http" || draft.transport === "sse";
        showToast(
          "error",
          remote
            ? zh
              ? "请填写有效的 HTTP/HTTPS 服务地址，并检查请求头为字符串对象。内容已保留。"
              : "Enter a valid HTTP/HTTPS server URL and a string-valued headers object. Your draft is preserved."
            : zh
              ? "请填写命令，并检查参数为字符串数组、环境变量为字符串对象。内容已保留。"
              : "Enter a command, a string array of arguments, and a string-valued environment object. Your draft is preserved.",
        );
        return false;
      }
      locked.current = true;
      setSaving(true);
      try {
        await write(config);
        return true;
      } catch {
        showToast(
          "error",
          zh
            ? "配置保存失败，编辑内容已保留。请检查文件状态后重试。"
            : "Could not save configuration. Your draft is preserved. Check the file and retry.",
        );
        return false;
      } finally {
        locked.current = false;
        setSaving(false);
      }
    },
    [zh],
  );
  const isSaving = useCallback(() => locked.current, []);
  return { saving, save, isSaving };
}
