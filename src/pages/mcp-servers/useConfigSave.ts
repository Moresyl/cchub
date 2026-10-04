import { useCallback, useRef, useState } from "react";
import { showToast } from "../../components/Toast";

interface Draft {
  name: string;
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
  const args: unknown = JSON.parse(draft.args);
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
  return { name: draft.name, command: draft.command.trim(), args, env: env as Record<string, string> };
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
        showToast(
          "error",
          zh
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
