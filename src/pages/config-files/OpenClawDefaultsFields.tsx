import { useState } from "react";
import { Plus, X } from "lucide-react";
import { getLocale } from "../../lib/i18n";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { showToast } from "../../components/Toast";
import { at, object, patchDocument, type NativeDocument, type OpenClawEditor } from "./useOpenClawEditor";
import OpenClawField from "./OpenClawField";

export default function OpenClawDefaultsFields({ root, editor }: { root: NativeDocument; editor: OpenClawEditor }) {
  const zh = getLocale() === "zh";
  const [newModel, setNewModel] = useState("");
  const path = ["agents", "defaults", "model"];
  const value = at(root, path);
  const fallbacks = object(value) && Array.isArray(value.fallbacks) ? value.fallbacks : [];
  const strings =
    (!object(value) || value.fallbacks === undefined || Array.isArray(value.fallbacks)) &&
    fallbacks.every((entry) => typeof entry === "string");
  function update(list: unknown[]) {
    try {
      if (typeof value === "string") editor.update(patchDocument(root, path, { primary: value, fallbacks: list }));
      else editor.update(patchDocument(root, [...path, "fallbacks"], list));
      return true;
    } catch {
      showToast(
        "error",
        zh
          ? "请先在原始配置中修正默认模型类型，草稿已保留。"
          : "Correct the default model in the raw configuration. Your draft is retained.",
      );
      return false;
    }
  }
  function add() {
    const name = newModel.trim();
    if (!name || fallbacks.includes(name)) return;
    if (update([...fallbacks, name])) setNewModel("");
  }
  return (
    <div className="min-w-0 rounded-lg border border-border p-4">
      <h4 className="mb-4 text-xs font-semibold">{zh ? "Agent 默认模型" : "Agent default model"}</h4>
      <OpenClawField
        root={root}
        editor={editor}
        path={typeof value === "string" ? path : [...path, "primary"]}
        label={zh ? "主模型" : "Primary model"}
        placeholder="provider-id/model-id"
      />
      <div className="mt-4 flex min-w-0 flex-col gap-2">
        <h5 className="text-xs font-medium text-muted-foreground">{zh ? "回退模型" : "Fallback models"}</h5>
        {strings ? (
          <>
            {fallbacks.map((entry, index) => (
              <div
                key={`${entry}-${index}`}
                className="flex min-w-0 items-center justify-between gap-2 rounded-md border border-border px-3"
              >
                <span className="min-w-0 break-all text-xs">{String(entry)}</span>
                <Button
                  variant="ghost"
                  aria-label={`${zh ? "移除回退模型" : "Remove fallback"} ${entry}`}
                  onClick={() => update(fallbacks.filter((_, position) => position !== index))}
                >
                  <X size={14} />
                </Button>
              </div>
            ))}
            <div className="flex min-w-0 gap-2">
              <Input
                aria-label={zh ? "新回退模型" : "New fallback model"}
                value={newModel}
                placeholder="provider-id/model-id"
                onChange={(event) => setNewModel(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                    event.preventDefault();
                    add();
                  }
                }}
              />
              <Button
                variant="secondary"
                disabled={!newModel.trim() || fallbacks.includes(newModel.trim())}
                onClick={add}
              >
                <Plus size={14} />
                {zh ? "添加回退" : "Add fallback"}
              </Button>
            </div>
          </>
        ) : (
          <OpenClawField
            root={root}
            editor={editor}
            path={[...path, "fallbacks"]}
            label={zh ? "原始回退列表" : "Original fallback list"}
          />
        )}
      </div>
    </div>
  );
}
