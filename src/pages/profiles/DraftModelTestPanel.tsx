import { useEffect, useId, useMemo, useState } from "react";
import { CheckCircle2, FlaskConical, Loader2, XCircle } from "lucide-react";
import ModelSelector, { type ModelInfo } from "../../components/ModelSelector";
import { Button } from "../../components/ui/button";
import { useDraftModelChecks } from "./draftModelChecks";
import { modelCheckMessage } from "./modelCheckMessage";

interface Props {
  scope: string;
  toolId: string;
  snapshot: string;
  model: string;
  configuredModels: string[];
  catalog: ModelInfo[];
  localeText: (zh: string, en: string, ja?: string) => string;
}

export default function DraftModelTestPanel({
  scope,
  toolId,
  snapshot,
  model,
  configuredModels,
  catalog,
  localeText,
}: Props) {
  const headingId = useId();
  const modelId = useId();
  const [selectedModel, setSelectedModel] = useState(model);
  useEffect(() => {
    setSelectedModel(model);
  }, [scope, model]);
  const models = [...new Set(configuredModels.map((id) => id.trim()).filter(Boolean))];
  const choices = useMemo(() => {
    const items = new Map(catalog.map((item) => [item.id, item]));
    for (const id of configuredModels) if (id.trim() && !items.has(id.trim())) items.set(id.trim(), { id: id.trim() });
    return [...items.values()];
  }, [catalog, configuredModels]);
  const { results, error, running, run } = useDraftModelChecks(
    scope,
    toolId,
    snapshot,
    JSON.stringify([selectedModel, models]),
  );
  const validDraft = useMemo(() => {
    try {
      const value: unknown = JSON.parse(snapshot);
      return !!value && typeof value === "object" && !Array.isArray(value);
    } catch {
      return false;
    }
  }, [snapshot]);
  if (!["claude", "codex", "gemini", "openclaw", "hermes", "opencode"].includes(toolId)) return null;

  return (
    <section aria-labelledby={headingId} className="min-w-0 rounded-lg border border-border bg-card p-3">
      <h3 id={headingId} className="mb-1 flex items-center gap-2 text-xs font-semibold">
        <FlaskConical size={14} aria-hidden="true" />{" "}
        {localeText("测试当前草稿", "Test current draft", "現在の下書きをテスト")}
      </h3>
      <p className="mb-3 text-xs leading-relaxed text-muted-foreground">
        {localeText(
          "使用当前草稿的密钥、账号、地址和请求设置发送最小模型请求，可能消耗少量额度。测试不会保存配置。",
          "Send minimal model requests using this draft’s key, account, URL and request settings. This may consume a small amount of quota. Testing does not save the profile.",
          "現在の下書きのキー、アカウント、URL とリクエスト設定で最小モデルリクエストを送信します。少量のクォータを消費する可能性があります。設定は保存されません。",
        )}
      </p>
      <label htmlFor={modelId} className="mb-1.5 block text-xs text-muted-foreground">
        {localeText("测试模型", "Model to test", "テストするモデル")}
      </label>
      <ModelSelector
        id={modelId}
        label={localeText("测试模型", "Model to test", "テストするモデル")}
        value={selectedModel}
        models={choices}
        onChange={setSelectedModel}
        placeholder={localeText("输入模型 ID", "Enter model ID", "モデル ID を入力")}
      />
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="outline"
          disabled={running || !validDraft || !selectedModel.trim()}
          onClick={() => void run([selectedModel.trim()])}
        >
          {running ? (
            <Loader2 size={14} className="animate-spin motion-reduce:animate-none" />
          ) : (
            <FlaskConical size={14} />
          )}
          {running
            ? localeText("测试中…", "Testing…", "テスト中…")
            : localeText("测试选定模型", "Test selected model", "選択したモデルをテスト")}
        </Button>
        {models.length > 1 && (
          <Button
            type="button"
            variant="outline"
            disabled={running || !validDraft || models.length > 32}
            onClick={() => void run(models)}
          >
            {localeText(
              `测试配置中的 ${models.length} 个模型`,
              `Test ${models.length} configured models`,
              `設定内の ${models.length} モデルをテスト`,
            )}
          </Button>
        )}
      </div>
      {!validDraft && (
        <p role="alert" className="mt-2 text-xs text-[var(--danger)]">
          {localeText(
            "请先修正原始配置中的 JSON 错误",
            "Correct the raw configuration JSON before testing",
            "テストの前に元の設定の JSON エラーを修正してください",
          )}
        </p>
      )}
      {models.length > 32 && (
        <p className="mt-2 text-xs text-muted-foreground">
          {localeText(
            "一次最多测试 32 个模型，请使用单个模型测试。",
            "Each batch supports up to 32 models. Use the individual model test.",
            "一括テストは最大 32 モデルです。個別テストを使用してください。",
          )}
        </p>
      )}
      {running && (
        <p role="status" className="mt-2 text-xs text-muted-foreground">
          {localeText(
            "正在等待模型完成响应…",
            "Waiting for complete model replies…",
            "モデルの完全な応答を待っています…",
          )}
        </p>
      )}
      {error && (
        <p role="alert" className="mt-2 break-words text-xs text-[var(--danger)]">
          {error}
        </p>
      )}
      {!!results.length && (
        <div className="mt-3 border-t border-border pt-3">
          <p role="status" className="mb-2 text-xs text-muted-foreground">
            {localeText(
              `测试完成：${results.filter((item) => item.status === "healthy").length}/${results.length} 个模型通过`,
              `Tests complete: ${results.filter((item) => item.status === "healthy").length}/${results.length} models passed`,
              `テスト完了：${results.filter((item) => item.status === "healthy").length}/${results.length} モデルが成功`,
            )}
          </p>
          <ul className="grid min-w-0 gap-2">
            {results.map((item) => {
              const healthy = item.status === "healthy";
              const Icon = healthy ? CheckCircle2 : XCircle;
              return (
                <li key={item.model} className="flex min-w-0 items-start gap-2 text-xs">
                  <Icon
                    size={14}
                    className="mt-0.5 shrink-0"
                    style={{ color: healthy ? "var(--success)" : "var(--danger)" }}
                    aria-hidden="true"
                  />
                  <div className="min-w-0 flex-1">
                    <p className="break-all">{item.model}</p>
                    <p className="mt-0.5 break-words text-muted-foreground">
                      {modelCheckMessage(item, localeText)}
                      {item.httpStatus !== null ? ` · HTTP ${item.httpStatus}` : ""}
                      {item.latencyMs !== null ? ` · ${item.latencyMs} ms` : ""}
                    </p>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </section>
  );
}
