import { useId, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { getLocale } from "../../lib/i18n";
import CodeEditor from "../../components/DeferredCodeEditor";
import ConfirmDialog from "../../components/ConfirmDialog";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { SimpleSelect } from "../../components/ui/simple-select";
import { showToast } from "../../components/Toast";
import OpenClawField from "./OpenClawField";
import OpenClawDefaultsFields from "./OpenClawDefaultsFields";
import { at, hasOwn, object, patchDocument, type NativeDocument, type OpenClawEditor } from "./useOpenClawEditor";

const protocols = [
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
  "bedrock-converse-stream",
];
export default function OpenClawNativeFields({
  editor,
  content,
  onChange,
  disabled,
}: {
  editor: OpenClawEditor;
  content: string;
  onChange: (content: string) => void;
  disabled: boolean;
}) {
  const zh = getLocale() === "zh";
  const text = (zhText: string, en: string) => (zh ? zhText : en);
  const id = useId();
  const [raw, setRaw] = useState(false);
  const [selectedProvider, setSelectedProvider] = useState("");
  const [selectedModel, setSelectedModel] = useState(0);
  const [providerName, setProviderName] = useState("");
  const [modelName, setModelName] = useState("");
  const [remove, setRemove] = useState<null | { kind: "provider" | "model"; name: string }>(null);
  const root = editor.data;
  const store = root ? at(root, ["models", "providers"]) : undefined;
  const providers = object(store) ? store : {};
  const providerId = hasOwn(providers, selectedProvider) ? selectedProvider : (Object.keys(providers)[0] ?? "");
  const provider =
    hasOwn(providers, providerId) && object(providers[providerId]) ? (providers[providerId] as NativeDocument) : null;
  const entries = provider && Array.isArray(provider.models) ? provider.models : [];
  const index = selectedModel < entries.length ? selectedModel : 0;
  const model = object(entries[index]) ? (entries[index] as NativeDocument) : null;
  const providerPath = ["models", "providers", providerId];
  const ref = model && typeof model.id === "string" ? `${providerId}/${model.id}` : "";
  const malformed =
    (store !== undefined && !object(store)) ||
    (!!provider && provider.models !== undefined && !Array.isArray(provider.models)) ||
    Object.values(providers).some((entry) => !object(entry));
  const blocked = disabled || editor.busy || editor.loading || editor.error || malformed;
  const field = (
    path: string[],
    label: string,
    options: { numeric?: boolean; secret?: boolean; placeholder?: string } = {},
  ) =>
    root && (
      <OpenClawField key={JSON.stringify(path)} editor={editor} root={root} path={path} label={label} {...options} />
    );
  const update = (path: string[], value: unknown) => {
    if (!root || blocked) return;
    try {
      editor.update(patchDocument(root, path, value));
    } catch {
      showToast(
        "error",
        text("请先在原始配置中修正字段类型。", "Correct the field type in the raw configuration first."),
      );
    }
  };
  async function showRaw(value: boolean) {
    if (disabled || editor.busy) return false;
    try {
      if (editor.hasPendingDraft) onChange(await editor.prepare());
      setRaw(value);
      return true;
    } catch {
      showToast(
        "error",
        text(
          "无法同步字段，请检查数值并保留当前草稿。",
          "Check the numeric fields before switching. Your draft is retained.",
        ),
      );
      return false;
    }
  }
  function addProvider() {
    const name = providerName.trim();
    if (!name || hasOwn(providers, name)) return;
    update(["models", "providers", name], { models: [] });
    setSelectedProvider(name);
    setSelectedModel(0);
    setProviderName("");
  }
  function addModel() {
    const name = modelName.trim();
    if (!provider || !name || entries.some((entry) => object(entry) && entry.id === name)) return;
    update([...providerPath, "models"], [...entries, { id: name, name }]);
    setSelectedModel(entries.length);
    setModelName("");
  }
  function deleteEntry() {
    if (!remove || !root || blocked) return;
    const numbers = editor.numbers.flatMap((path) => {
      if (path[2] !== providerId) return [path];
      if (remove.kind === "provider" || Number(path[4]) === index) return [];
      return [Number(path[4]) > index ? [...path.slice(0, 4), String(Number(path[4]) - 1), ...path.slice(5)] : path];
    });
    const next =
      remove.kind === "provider"
        ? patchDocument(root, providerPath, undefined)
        : patchDocument(
            root,
            [...providerPath, "models"],
            entries.filter((_, position) => position !== index),
          );
    editor.update(next, numbers);
    setRemove(null);
    setSelectedModel(0);
  }
  return (
    <div className="flex min-w-0 flex-col gap-4">
      <div
        role="tablist"
        aria-label={text("配置编辑方式", "Configuration editor mode")}
        className="flex gap-1 rounded-lg border border-border bg-[var(--bg-elevated)] p-1"
      >
        {[false, true].map((value, position) => (
          <Button
            key={String(value)}
            id={`${id}-tab-${position}`}
            role="tab"
            aria-selected={raw === value}
            aria-controls={`${id}-panel-${position}`}
            tabIndex={raw === value ? 0 : -1}
            variant={raw === value ? "secondary" : "ghost"}
            disabled={disabled || editor.busy}
            onClick={() => void showRaw(value)}
            onKeyDown={(event) => {
              if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
                event.preventDefault();
                const next = event.key === "Home" ? false : event.key === "End" ? true : !raw;
                void showRaw(next).then((changed) =>
                  document.getElementById(`${id}-tab-${(changed ? next : raw) ? 1 : 0}`)?.focus(),
                );
              }
            }}
          >
            {text(value ? "原始 JSON5" : "结构化字段", value ? "Raw JSON5" : "Structured fields")}
          </Button>
        ))}
      </div>
      {editor.error && (
        <p role="alert" className="text-xs text-destructive">
          {text(
            "原生 JSON5 解析失败。草稿已保留，请切换原始配置修正语法或重复字段。",
            "Correct the raw JSON5 syntax or duplicate fields. Your draft is retained.",
          )}
        </p>
      )}
      {editor.invalidNumber && (
        <p role="alert" className="text-xs text-destructive">
          {text(
            "成本需为非负数，上下文窗口需为正整数。",
            "Costs must be nonnegative and the context window a positive integer.",
          )}
        </p>
      )}
      <div id={`${id}-panel-0`} role="tabpanel" aria-labelledby={`${id}-tab-0`} hidden={raw}>
        {editor.loading ? (
          <p role="status" className="text-xs text-muted-foreground">
            {text("正在读取原生字段…", "Reading native fields…")}
          </p>
        ) : malformed ? (
          <p role="alert" className="text-xs text-destructive">
            {text(
              "供应商或模型容器类型不正确，请在原始配置中修复。",
              "Repair the provider or model container in the raw configuration.",
            )}
          </p>
        ) : (
          root && (
            <fieldset disabled={blocked} className="flex min-w-0 flex-col gap-5">
              <div className="grid min-w-0 gap-4 lg:grid-cols-[180px_minmax(0,1fr)]">
                <div className="flex min-w-0 flex-col gap-2 rounded-lg border border-border p-3">
                  <h4 className="text-xs font-semibold">{text("供应商", "Providers")}</h4>
                  {Object.keys(providers).map((name) => (
                    <Button
                      key={name}
                      variant={providerId === name ? "secondary" : "ghost"}
                      className="h-auto min-h-8 justify-start whitespace-normal break-all text-left"
                      aria-pressed={providerId === name}
                      onClick={() => {
                        setSelectedProvider(name);
                        setSelectedModel(0);
                      }}
                    >
                      {name}
                    </Button>
                  ))}
                  {!Object.keys(providers).length && (
                    <p className="text-xs text-muted-foreground">{text("尚未配置供应商", "No providers configured")}</p>
                  )}
                  <Input
                    aria-label={text("新供应商 ID", "New provider ID")}
                    value={providerName}
                    onChange={(event) => setProviderName(event.target.value)}
                    placeholder="provider-id"
                    onKeyDown={(event) => {
                      if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                        event.preventDefault();
                        addProvider();
                      }
                    }}
                  />
                  <Button
                    variant="secondary"
                    disabled={!providerName.trim() || hasOwn(providers, providerName.trim())}
                    onClick={addProvider}
                  >
                    <Plus size={14} />
                    {text("添加供应商", "Add provider")}
                  </Button>
                </div>
                <div className="min-w-0 rounded-lg border border-border p-4">
                  {provider ? (
                    <div className="flex min-w-0 flex-col gap-4">
                      <div className="flex min-w-0 items-center justify-between gap-2">
                        <h4 className="break-all text-sm font-semibold">{providerId}</h4>
                        <Button
                          variant="ghost"
                          aria-label={text("删除供应商", "Delete provider")}
                          onClick={() => setRemove({ kind: "provider", name: providerId })}
                        >
                          <Trash2 size={14} />
                        </Button>
                      </div>
                      <div className="grid min-w-0 gap-4 sm:grid-cols-2">
                        {field([...providerPath, "baseUrl"], text("接口地址", "Base URL"), {
                          placeholder: "https://api.example.com/v1",
                        })}
                        {field([...providerPath, "apiKey"], "API Key", { secret: true })}
                        <div className="flex min-w-0 flex-col gap-2">
                          <label className="text-xs font-medium text-muted-foreground" htmlFor={`${id}-protocol`}>
                            {text("接口协议", "API protocol")}
                          </label>
                          <SimpleSelect
                            id={`${id}-protocol`}
                            value={typeof provider.api === "string" ? provider.api : ""}
                            ariaLabel={text("接口协议", "API protocol")}
                            options={[
                              ...new Set([
                                "",
                                ...protocols,
                                ...(typeof provider.api === "string" ? [provider.api] : []),
                              ]),
                            ].map((value) => ({ value, label: value || text("使用默认协议", "Default protocol") }))}
                            onValueChange={(value) => update([...providerPath, "api"], value || undefined)}
                          />
                        </div>
                      </div>
                      <div className="border-t border-border pt-4">
                        <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                          <h4 className="text-xs font-semibold">{text("模型", "Models")}</h4>
                          {model && (
                            <Button
                              variant="ghost"
                              aria-label={text("删除模型", "Delete model")}
                              onClick={() => setRemove({ kind: "model", name: String(model.id ?? index) })}
                            >
                              <Trash2 size={14} />
                            </Button>
                          )}
                        </div>
                        {!!entries.length && (
                          <SimpleSelect
                            value={String(index)}
                            ariaLabel={text("选择模型", "Select model")}
                            options={entries.map((entry, position) => ({
                              value: String(position),
                              label: object(entry)
                                ? String(entry.name ?? entry.id ?? position + 1)
                                : String(position + 1),
                            }))}
                            onValueChange={(value) => setSelectedModel(Number(value))}
                          />
                        )}
                        {model && (
                          <div className="mt-4 grid min-w-0 gap-4 sm:grid-cols-2">
                            {field([...providerPath, "models", String(index), "id"], text("模型 ID", "Model ID"))}
                            {field([...providerPath, "models", String(index), "name"], text("显示名", "Display name"))}
                            {field(
                              [...providerPath, "models", String(index), "contextWindow"],
                              text("上下文窗口", "Context window"),
                              { numeric: true },
                            )}
                            {ref &&
                              field(["agents", "defaults", "models", ref, "alias"], text("模型别名", "Model alias"))}
                            {field(
                              [...providerPath, "models", String(index), "cost", "input"],
                              text("输入成本", "Input cost"),
                              { numeric: true },
                            )}
                            {field(
                              [...providerPath, "models", String(index), "cost", "output"],
                              text("输出成本", "Output cost"),
                              { numeric: true },
                            )}
                          </div>
                        )}
                        <div className="mt-4 flex min-w-0 gap-2">
                          <Input
                            aria-label={text("新模型 ID", "New model ID")}
                            value={modelName}
                            onChange={(event) => setModelName(event.target.value)}
                            placeholder="model-id"
                            onKeyDown={(event) => {
                              if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                                event.preventDefault();
                                addModel();
                              }
                            }}
                          />
                          <Button
                            variant="secondary"
                            disabled={
                              !modelName.trim() ||
                              entries.some((entry) => object(entry) && entry.id === modelName.trim())
                            }
                            onClick={addModel}
                          >
                            <Plus size={14} />
                            {text("添加", "Add")}
                          </Button>
                        </div>
                      </div>
                    </div>
                  ) : (
                    <p className="text-xs text-muted-foreground">
                      {text(
                        "选择或添加一个供应商以编辑连接和模型。",
                        "Select or add a provider to edit its connection and models.",
                      )}
                    </p>
                  )}
                </div>
              </div>
              <OpenClawDefaultsFields root={root} editor={editor} />
            </fieldset>
          )
        )}
      </div>
      <div id={`${id}-panel-1`} role="tabpanel" aria-labelledby={`${id}-tab-1`} hidden={!raw} className="min-w-0">
        <CodeEditor
          value={content}
          onChange={onChange}
          language="json5"
          minHeight={320}
          maxHeight={700}
          readOnly={disabled || editor.busy}
          ariaLabel={text("OpenClaw 原始配置", "OpenClaw raw configuration")}
        />
      </div>
      <ConfirmDialog
        isOpen={!!remove}
        title={text("删除配置条目", "Delete configuration entry")}
        message={text(
          `移除 ${remove?.name ?? ""}。更改将在保存文件后生效，请同时检查主模型与回退模型是否引用此条目。`,
          `Remove ${remove?.name ?? ""}. Save the file to apply this change. Check whether the primary or fallback models reference this entry.`,
        )}
        onConfirm={deleteEntry}
        onCancel={() => setRemove(null)}
      />
    </div>
  );
}
