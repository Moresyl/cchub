import { useEffect, useId, useRef, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import type { StructuredDraftFields } from "../../lib/configProfiles";
import { previewModelAlias, validateModelAliases } from "../../lib/configProfiles/modelAliases";
import { modelAliasValidationMessage } from "./modelAliasValidation";

interface Props {
  fields: StructuredDraftFields;
  localeText: (zh: string, en: string, ja?: string) => string;
  onChange: (next: Partial<StructuredDraftFields>) => void;
}

export function ModelAliasesSection({ fields, localeText: t, onChange }: Props) {
  const id = useId();
  const inputRefs = useRef(new Map<number, HTMLInputElement>());
  const addButton = useRef<HTMLButtonElement>(null);
  const [focusRow, setFocusRow] = useState<number | null>(null);
  const rows = fields.localProxyModelAliases ?? [];
  const error = validateModelAliases(rows, fields.localProxyModelAliasesRaw);
  const model = fields.model.trim();
  const preview = !error && model ? previewModelAlias(rows, model) : null;
  useEffect(() => {
    if (focusRow !== null && (rows.length === 0 || inputRefs.current.has(focusRow))) {
      if (rows.length === 0) addButton.current?.focus();
      else inputRefs.current.get(focusRow)?.focus();
      setFocusRow(null);
    }
  }, [focusRow, rows.length]);
  const update = (index: number, key: "model" | "upstream", value: string) =>
    onChange({ localProxyModelAliases: rows.map((row, i) => (i === index ? { ...row, [key]: value } : row)) });
  const message = modelAliasValidationMessage(fields, t, model);
  return (
    <div className="mt-4 space-y-3 border-t border-[var(--border-subtle)] pt-4" aria-labelledby={`${id}-title`}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h4 id={`${id}-title`} className="text-xs font-[590]">
          {t("模型别名 · 本地代理", "Model aliases · Local proxy", "モデル別名 · ローカルプロキシ")}
        </h4>
        {fields.localProxyModelAliasesRaw === undefined && (
          <Button
            ref={addButton}
            type="button"
            variant="secondary"
            disabled={rows.length >= 128}
            onClick={() => {
              setFocusRow(rows.length);
              onChange({ localProxyModelAliases: [...rows, { model: "", upstream: "" }] });
            }}
          >
            <Plus size={14} />
            {t("添加别名", "Add alias", "別名を追加")}
          </Button>
        )}
      </div>
      <p className="text-xs leading-relaxed text-muted-foreground">
        {t(
          "仅在启用本地代理时生效。完整名称优先于 *；供应商名称中的每个 * 都替换为请求模型。不会更改模型目录或路由规则。",
          "Applies only through the local proxy. Exact names override *. Every * in the upstream name expands to the requested model. Catalog and routing names stay the same.",
          "ローカルプロキシのみで適用。完全一致が * より優先され、転送名の * はリクエストモデルに展開されます。",
        )}
      </p>
      {rows.map((row, index) => (
        <div key={index} className="grid grid-cols-[minmax(0,1fr)_auto] items-end gap-2">
          <div className="grid min-w-0 grid-cols-1 gap-2 sm:grid-cols-2">
            {(["model", "upstream"] as const).map((key) => (
              <div key={key} className="min-w-0 space-y-1">
                <label htmlFor={`${id}-${index}-${key}`} className="text-[11px] text-muted-foreground">
                  {key === "model"
                    ? t(`请求模型 ${index + 1}`, `Requested model ${index + 1}`, `リクエストモデル ${index + 1}`)
                    : t(`供应商模型 ${index + 1}`, `Upstream model ${index + 1}`, `転送モデル ${index + 1}`)}
                </label>
                <Input
                  id={`${id}-${index}-${key}`}
                  value={row[key]}
                  placeholder={key === "model" ? "model-id / *" : "vendor/*"}
                  ref={
                    key === "model"
                      ? (element) => {
                          if (element) inputRefs.current.set(index, element);
                          else inputRefs.current.delete(index);
                        }
                      : undefined
                  }
                  aria-invalid={error?.row === index + 1 || undefined}
                  aria-describedby={message ? `${id}-error` : undefined}
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(event) => update(index, key, event.target.value)}
                />
              </div>
            ))}
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={t(`删除别名 ${index + 1}`, `Remove alias ${index + 1}`, `別名 ${index + 1} を削除`)}
            onClick={() => {
              setFocusRow(Math.max(0, Math.min(index, rows.length - 2)));
              onChange({ localProxyModelAliases: rows.filter((_, i) => i !== index) });
            }}
          >
            <Trash2 size={14} />
          </Button>
        </div>
      ))}
      {message && (
        <p id={`${id}-error`} role="alert" className="text-xs leading-relaxed text-destructive">
          {message}
        </p>
      )}
      {fields.localProxyModelAliasesRaw !== undefined && (
        <Button
          type="button"
          variant="secondary"
          onClick={() => {
            setFocusRow(0);
            onChange({ localProxyModelAliases: [], localProxyModelAliasesRaw: undefined });
          }}
        >
          {t("清空损坏的别名", "Clear malformed aliases", "不正な別名をクリア")}
        </Button>
      )}
      {!message && rows.length > 0 && preview && (
        <p className="break-all text-xs leading-relaxed text-muted-foreground">
          {t("当前模型转发预览：", "Current model wire preview: ", "現在のモデルの転送名：")}
          <span className="font-mono text-foreground">
            {model} → {preview}
          </span>
        </p>
      )}
    </div>
  );
}
