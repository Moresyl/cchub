import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ModelInfo } from "../../components/ModelSelector";
import { showToast } from "../../components/Toast";
import { formatModelFetchError, supportsModelFetch } from "./helpers";

export interface ModelDiscoveryContext {
  fetchingModels: boolean;
  draftTool: string;
  draftProviderType: string;
  draftOAuthAccountId: string;
  draftApiKey: string;
  draftUseFullUrl: boolean;
  draftBaseUrl: string;
  draftCustomUserAgent: string;
  draftRequestHeaders: Record<string, string>;
  localeText: (zh: string, en: string, ja?: string) => string;
  setFetchingModels: (value: boolean) => void;
  setModelFetchError: (value: string | null) => void;
  setFetchedModelDetails: (value: ModelInfo[]) => void;
  setFetchedModels: (value: string[]) => void;
  isCurrent?: () => boolean;
}

async function fetchCatalog(ctx: ModelDiscoveryContext): Promise<ModelInfo[]> {
  const accountId = ctx.draftOAuthAccountId.trim() || null;
  if (ctx.draftProviderType === "github_copilot") {
    const models = await invoke<Array<{ id: string; name: string }>>("copilot_get_models", { accountId });
    return models.map((model) => ({ id: model.id, displayName: model.name }));
  }
  if (ctx.draftProviderType === "codex_oauth" || ctx.draftProviderType === "xai_oauth") {
    return invoke<ModelInfo[]>(
      ctx.draftProviderType === "codex_oauth" ? "get_codex_oauth_models" : "get_xai_oauth_models",
      { accountId },
    );
  }
  return invoke<ModelInfo[]>("fetch_provider_models_detailed", {
    toolId: ctx.draftTool,
    baseUrl: ctx.draftBaseUrl,
    apiKey: ctx.draftApiKey,
    useFullUrl: ctx.draftUseFullUrl,
    customUserAgent: ctx.draftCustomUserAgent,
    requestHeaders: ctx.draftRequestHeaders,
  });
}

export async function performFetchModels(ctx: ModelDiscoveryContext): Promise<void> {
  if (ctx.fetchingModels || !supportsModelFetch(ctx.draftTool)) return;
  const oauth = ["github_copilot", "codex_oauth", "xai_oauth"].includes(ctx.draftProviderType);
  if (!oauth && !ctx.draftApiKey.trim()) {
    showToast(
      "error",
      ctx.localeText(
        "请先填写 API Key，再拉取模型列表",
        "Enter an API key before fetching models",
        "モデル一覧を取得する前に API Key を入力してください",
      ),
    );
    return;
  }
  if (!oauth && ctx.draftUseFullUrl && !ctx.draftBaseUrl.trim()) {
    showToast(
      "error",
      ctx.localeText(
        "完整端点模式下需要填写完整接口地址",
        "Full endpoint mode requires a complete endpoint URL",
        "完全なエンドポイントモードでは完全な URL が必要です",
      ),
    );
    return;
  }
  const isCurrent = ctx.isCurrent ?? (() => true);
  ctx.setFetchingModels(true);
  ctx.setModelFetchError(null);
  try {
    const result = await fetchCatalog(ctx);
    if (!isCurrent()) return;
    // Some gateways repeat IDs. Keep one entry, enriching it with later metadata.
    const unique = new Map<string, ModelInfo>();
    for (const model of result) {
      const id = model.id?.trim();
      if (!id) continue;
      const previous = unique.get(id);
      const details = Object.fromEntries(Object.entries(model).filter(([, value]) => value != null && value !== ""));
      unique.set(id, { ...previous, ...details, id });
    }
    const models = [...unique.values()];
    ctx.setFetchedModelDetails(models);
    ctx.setFetchedModels(models.map((model) => model.id));
    showToast(
      "success",
      models.length
        ? ctx.localeText(
            `已发现 ${models.length} 个模型`,
            `Discovered ${models.length} models`,
            `${models.length} 個のモデルを検出しました`,
          )
        : ctx.localeText(
            "已连接成功，但供应商没有返回可选模型",
            "Connected successfully, but the provider returned no models",
            "接続には成功しましたが、プロバイダーは利用可能なモデルを返しませんでした",
          ),
    );
  } catch (error) {
    if (!isCurrent()) return;
    const message = formatModelFetchError(error, ctx.localeText);
    ctx.setModelFetchError(message);
    // A failed refresh should not erase a previously usable catalog.
    showToast("error", message);
  } finally {
    if (isCurrent()) ctx.setFetchingModels(false);
  }
}

export function useModelDiscovery(ctx: ModelDiscoveryContext, editorScope: string): () => Promise<void> {
  const generation = useRef(0);
  const inFlight = useRef(false);
  const identity = JSON.stringify([
    editorScope,
    ctx.draftTool,
    ctx.draftProviderType,
    ctx.draftOAuthAccountId,
    ctx.draftApiKey,
    ctx.draftBaseUrl,
    ctx.draftUseFullUrl,
    ctx.draftCustomUserAgent,
    Object.entries(ctx.draftRequestHeaders).sort(([a], [b]) => a.localeCompare(b)),
  ]);
  const { setFetchingModels, setFetchedModels, setFetchedModelDetails, setModelFetchError } = ctx;

  useEffect(() => {
    generation.current += 1;
    inFlight.current = false;
    setFetchingModels(false);
    setFetchedModels([]);
    setFetchedModelDetails([]);
    setModelFetchError(null);
    return () => {
      generation.current += 1;
      inFlight.current = false;
    };
  }, [identity, setFetchingModels, setFetchedModels, setFetchedModelDetails, setModelFetchError]);

  return useCallback(async () => {
    if (inFlight.current) return;
    const requestGeneration = generation.current;
    inFlight.current = true;
    try {
      await performFetchModels({ ...ctx, isCurrent: () => generation.current === requestGeneration });
    } finally {
      if (generation.current === requestGeneration) inFlight.current = false;
    }
  }, [ctx]);
}
