import { useId } from "react";
import CodeEditor from "../../components/DeferredCodeEditor";
import { Input } from "../../components/ui/input";
import { getLocale } from "../../lib/i18n";
import { showToast } from "../../components/Toast";
import { at, object, patchDocument, type NativeDocument, type OpenClawEditor } from "./useOpenClawEditor";

export default function OpenClawField({
  editor,
  root,
  path,
  label,
  numeric = false,
  secret = false,
  placeholder,
}: {
  editor: OpenClawEditor;
  root: NativeDocument;
  path: string[];
  label: string;
  numeric?: boolean;
  secret?: boolean;
  placeholder?: string;
}) {
  const id = useId();
  const value = at(root, path);
  const structured =
    object(value) ||
    Array.isArray(value) ||
    (value !== undefined && value !== null && typeof value !== "string" && typeof value !== "number");
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <label htmlFor={id} className="text-xs font-medium text-muted-foreground">
        {label}
      </label>
      {structured ? (
        <CodeEditor
          value={JSON.stringify(value, null, 2)}
          language="json"
          readOnly
          minHeight={80}
          maxHeight={140}
          ariaLabel={label}
        />
      ) : (
        <Input
          id={id}
          value={value === undefined || value === null ? "" : String(value)}
          type={secret ? "password" : "text"}
          inputMode={numeric ? "decimal" : undefined}
          placeholder={placeholder}
          onChange={(event) => {
            try {
              if (numeric) editor.updateNumber(path, event.target.value);
              else editor.update(patchDocument(root, path, event.target.value || undefined));
            } catch {
              showToast(
                "error",
                getLocale() === "zh"
                  ? "请先在原始配置中修正字段容器类型，草稿已保留。"
                  : "Correct the containing field in the raw configuration. Your draft is retained.",
              );
            }
          }}
        />
      )}
    </div>
  );
}
