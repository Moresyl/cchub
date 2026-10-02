import { memo, useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, RefreshCw, Save } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import CodeEditor from "./CodeEditor";
import CollapsibleSection from "./CollapsibleSection";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { Input } from "./ui/input";
import { SimpleSelect } from "./ui/simple-select";
import {
  createDefaultStructuredFields,
  parseStructuredConfig,
  type StructuredDraftFields,
} from "../lib/configProfiles";

type HermesFieldKey = "baseUrl" | "apiKey" | "model" | "hermesProvider" | "hermesApiKeyEnv";
const PROVIDERS = ["nous", "openrouter", "gemini", "zai", "kimi-coding", "anthropic", "custom"];

function formContent(draft: StructuredDraftFields, maskKey = false, previousKey = ""): string {
  const key = draft.hermesApiKeyEnv.trim();
  const env = key ? { [key]: maskKey && draft.apiKey.trim() ? "••••••••" : draft.apiKey.trim() } : {};
  if (previousKey && previousKey !== key) env[previousKey] = "";
  return JSON.stringify(
    {
      config: {
        model: {
          provider: draft.hermesProvider.trim() || "custom",
          base_url: draft.baseUrl.trim(),
          default: draft.model.trim(),
        },
      },
      env,
      metadata: { hermesApiKeyEnv: key || undefined },
    },
    null,
    2,
  );
}

function readForm(content: string): StructuredDraftFields {
  const invalid = () => new Error("Invalid Hermes configuration snapshot");
  const object = (value: unknown): value is Record<string, unknown> =>
    !!value && typeof value === "object" && !Array.isArray(value);
  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch {
    throw invalid();
  }
  if (!object(parsed) || !object(parsed.config) || !object(parsed.env)) throw invalid();
  const model = parsed.config.model === undefined ? {} : parsed.config.model;
  const metadata = parsed.metadata ?? {};
  if (!object(model) || !object(metadata) || Object.values(parsed.env).some((value) => typeof value !== "string"))
    throw invalid();
  for (const name of ["provider", "base_url", "default"])
    if (model[name] !== undefined && typeof model[name] !== "string") throw invalid();
  for (const name of ["hermesApiKeyEnv", "hermesProvider"])
    if (metadata[name] != null && typeof metadata[name] !== "string") throw invalid();
  const envKey = typeof metadata.hermesApiKeyEnv === "string" ? metadata.hermesApiKeyEnv : "";
  return {
    ...parseStructuredConfig("hermes", content),
    baseUrl: (model.base_url as string | undefined) ?? "",
    model: (model.default as string | undefined) ?? "",
    hermesProvider: (model.provider as string | undefined) ?? "",
    hermesApiKeyEnv: envKey,
    apiKey: (parsed.env[envKey] as string | undefined) ?? "",
  };
}

function HermesConfigSectionComponent() {
  const locale = getLocale();
  const fieldId = useId();
  const uiText = useCallback(
    (zh: string, en: string, ja?: string) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en),
    [locale],
  );
  const [draft, setDraft] = useState<StructuredDraftFields>(() => createDefaultStructuredFields("hermes"));
  const [phase, setPhase] = useState<"loading" | "ready" | "failed" | "saving">("loading");
  const [error, setError] = useState("");
  const [baseline, setBaseline] = useState<string | null>(null);
  const [previousKey, setPreviousKey] = useState("");
  const [rootOverride, setRootOverride] = useState<string | null>(null);
  const pending = useRef(false);
  const mounted = useRef(false);
  const disabled = phase !== "ready";
  const content = useMemo(() => formContent(draft, false, previousKey), [draft, previousKey]);
  const dirty = baseline !== null && content !== baseline;
  const keyError =
    (draft.apiKey.trim() && !draft.hermesApiKeyEnv.trim()) ||
    (draft.hermesApiKeyEnv.trim() && !/^[A-Za-z_][A-Za-z0-9_]*$/.test(draft.hermesApiKeyEnv.trim()))
      ? uiText(
          "请填写有效的密钥环境变量名，如 PROVIDER_API_KEY。",
          "Enter a valid key variable name, such as PROVIDER_API_KEY.",
        )
      : "";

  const updateDraft = useCallback((field: HermesFieldKey, value: string) => {
    setDraft((current) => ({ ...current, [field]: value }));
  }, []);

  const loadConfig = useCallback(async () => {
    if (pending.current) return;
    pending.current = true;
    setPhase("loading");
    setError("");
    try {
      const [content, override] = await Promise.all([
        invoke<string>("read_tool_config", { toolId: "hermes" }),
        invoke<string | null>("get_hermes_root_override"),
      ]);
      if (!mounted.current) return;
      const loaded = readForm(content);
      setDraft(loaded);
      setBaseline(formContent(loaded));
      setPreviousKey(loaded.hermesApiKeyEnv.trim());
      setRootOverride(override);
      setPhase("ready");
    } catch (failure) {
      if (!mounted.current) return;
      setError(String(failure));
      setPhase("failed");
    } finally {
      pending.current = false;
    }
  }, []);

  const saveConfig = useCallback(async () => {
    if (pending.current || phase !== "ready" || !dirty || keyError) return;
    pending.current = true;
    setPhase("saving");
    setError("");
    try {
      await invoke("write_tool_config", {
        toolId: "hermes",
        content,
      });
      if (mounted.current) {
        setPreviousKey(draft.hermesApiKeyEnv.trim());
        setBaseline(formContent(draft));
        showToast("success", uiText("Hermes 配置已保存", "Hermes configuration saved"));
      }
    } catch (failure) {
      if (mounted.current) setError(String(failure));
    } finally {
      pending.current = false;
      if (mounted.current) setPhase("ready");
    }
  }, [content, dirty, draft, keyError, phase, uiText]);

  useEffect(() => {
    mounted.current = true;
    void loadConfig();
    return () => {
      mounted.current = false;
    };
  }, [loadConfig]);

  const preview = useMemo(() => formContent(draft, true, previousKey), [draft, previousKey]);
  const providerOptions = useMemo(
    () => [...new Set([...PROVIDERS, draft.hermesProvider].filter(Boolean))].map((value) => ({ value, label: value })),
    [draft.hermesProvider],
  );
  const inputs: { key: HermesFieldKey; label: string; placeholder: string; password?: boolean }[] = [
    { key: "hermesApiKeyEnv", label: uiText("密钥环境变量", "API key variable"), placeholder: "PROVIDER_API_KEY" },
    { key: "baseUrl", label: uiText("API 地址", "API URL"), placeholder: "https://api.example.com/v1" },
    { key: "model", label: uiText("默认模型", "Default model"), placeholder: "model-id" },
    {
      key: "apiKey",
      label: uiText("API 密钥", "API key"),
      placeholder: uiText("输入 API 密钥", "Enter API key"),
      password: true,
    },
  ];

  return (
    <Card className="mb-4 min-w-0 shadow-none" aria-busy={phase === "loading" || phase === "saving"}>
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0 space-y-2">
            <CardTitle>{uiText("Hermes 配置", "Hermes configuration")}</CardTitle>
            <CardDescription>
              {uiText(
                "修改模型连接，保留已有 YAML 注释和其他设置。",
                "Update the model connection while retaining YAML comments and other settings.",
              )}
            </CardDescription>
            {rootOverride && (
              <p className="break-all text-xs text-muted-foreground">
                {uiText("根目录：", "Root: ")}
                {rootOverride}
              </p>
            )}
          </div>
          <div className="flex shrink-0 gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => void loadConfig()}
              disabled={phase === "loading" || phase === "saving"}
            >
              <RefreshCw size={14} aria-hidden="true" />
              {uiText("重新读取", "Reload")}
            </Button>
            <Button type="button" onClick={() => void saveConfig()} disabled={disabled || !dirty || !!keyError}>
              {phase === "saving" ? (
                <Loader2 size={14} className="animate-spin" aria-hidden="true" />
              ) : (
                <Save size={14} aria-hidden="true" />
              )}
              {uiText("保存配置", "Save configuration")}
            </Button>
          </div>
        </div>
        {phase === "loading" && (
          <p role="status" className="text-xs text-muted-foreground">
            {uiText("正在读取配置…", "Reading configuration…")}
          </p>
        )}
        {error && (
          <div
            role="alert"
            className="rounded-md border border-[var(--danger)]/30 bg-[var(--danger-subtle)] p-3 text-xs text-[var(--danger)]"
          >
            <p>
              {phase === "failed"
                ? uiText("读取失败，请重新读取后再编辑。", "Reading failed. Reload before editing.")
                : uiText("保存失败，已保留输入内容。", "Saving failed. Your draft has been retained.")}
            </p>
            <p className="mt-1 break-words">{error}</p>
          </div>
        )}
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,320px),1fr))] gap-4">
          <div className="min-w-0 space-y-1.5">
            <label className="field-label" htmlFor={`${fieldId}-provider`}>
              {uiText("供应商", "Provider")}
            </label>
            <SimpleSelect
              id={`${fieldId}-provider`}
              value={draft.hermesProvider}
              onValueChange={(value) => updateDraft("hermesProvider", value)}
              options={[{ value: "", label: uiText("未配置", "Not configured") }, ...providerOptions]}
              ariaLabel={uiText("供应商", "Provider")}
              disabled={disabled}
            />
          </div>
          {inputs.map((field) => (
            <div key={field.key} className={`min-w-0 space-y-1.5${field.password ? " col-span-full" : ""}`}>
              <label className="field-label" htmlFor={`${fieldId}-${field.key}`}>
                {field.label}
              </label>
              <Input
                id={`${fieldId}-${field.key}`}
                type={field.password ? "password" : "text"}
                value={draft[field.key]}
                onChange={(event) => updateDraft(field.key, event.target.value)}
                placeholder={field.placeholder}
                disabled={disabled}
                autoComplete={field.password ? "new-password" : "off"}
                spellCheck={false}
                aria-invalid={field.key === "hermesApiKeyEnv" && !!keyError ? true : undefined}
                aria-describedby={field.key === "hermesApiKeyEnv" && keyError ? `${fieldId}-key-error` : undefined}
              />
              {field.key === "hermesApiKeyEnv" && keyError && (
                <p id={`${fieldId}-key-error`} className="text-xs text-[var(--danger)]">
                  {keyError}
                </p>
              )}
            </div>
          ))}
        </div>
        {phase !== "loading" && phase !== "failed" && (
          <div className="border-t border-border pt-3">
            <CollapsibleSection
              title={uiText("配置预览", "Configuration preview")}
              summary={uiText("JSON · 密钥已遮盖", "JSON · Key masked")}
            >
              <CodeEditor
                value={preview}
                language="json"
                readOnly
                minHeight={120}
                maxHeight={260}
                ariaLabel={uiText("配置预览", "Configuration preview")}
              />
            </CollapsibleSection>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default memo(HermesConfigSectionComponent);
