/* eslint-disable @typescript-eslint/no-explicit-any */
import { useEffect, useMemo, useRef, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";

import { showToast } from "../../components/Toast";
import { type ModelInfo } from "../../components/ModelSelector";
import {
  buildStructuredConfig,
  createDefaultStructuredFields,
  parseStructuredConfig,
  supportsStructuredConfig,
  type StructuredDraftFields,
} from "../../lib/configProfiles";

import {
  formatModelFetchError,
  prettyJson,
  supportsModelFetch,
  type ConfigProfile,
  type ProviderStreamCheckResult,
} from "./helpers";
import { mergeSharedDraftFields } from "./draftMerge";

type LocaleText = (zh: string, en: string, ja?: string) => string;

interface BatchStreamCheckContext {
  localeText: LocaleText;
  setChecking: (value: boolean) => void;
  setResults: Dispatch<SetStateAction<Record<string, ProviderStreamCheckResult>>>;
}

export async function performBatchStreamCheck(ctx: BatchStreamCheckContext): Promise<void> {
  if (localStorage.getItem("cchub-stream-check-confirmed") !== "1") {
    const confirmed = window.confirm(
      ctx.localeText(
        "全量流检会向每个配置的供应商发送最小流式请求，可能消耗额度。继续吗？",
        "Batch stream checks send a minimal streaming request to every configured provider and may consume quota. Continue?",
        "全体ストリーム確認は各 Provider に最小ストリームリクエストを送り、クォータを消費する場合があります。続行しますか？",
      ),
    );
    if (!confirmed) return;
    localStorage.setItem("cchub-stream-check-confirmed", "1");
  }

  ctx.setChecking(true);
  try {
    const results = await invoke<ProviderStreamCheckResult[]>("stream_check_all_config_profiles");
    ctx.setResults((current) => ({
      ...current,
      ...Object.fromEntries(results.map((result) => [result.profile_id, result])),
    }));
    const healthy = results.filter((result) => result.status === "healthy").length;
    showToast(
      "success",
      ctx.localeText(
        `全量流检完成：${healthy}/${results.length} 健康`,
        `Batch stream check complete: ${healthy}/${results.length} healthy`,
        `全体ストリーム確認完了: ${healthy}/${results.length} healthy`,
      ),
    );
  } catch (error) {
    showToast(
      "error",
      ctx.localeText(
        `全量流检失败: ${error}`,
        `Batch stream check failed: ${error}`,
        `全体ストリーム確認に失敗: ${error}`,
      ),
    );
  } finally {
    ctx.setChecking(false);
  }
}

// 把页面里十几个 boolean / list state 的 setter 一起塞进来；
// 不在这里 useCallback，因为 caller 已经把 deps 锁定好了。
interface FetchModelsContext {
  fetchingModels: boolean;
  draftTool: string;
  draftProviderType: string;
  draftOAuthAccountId: string;
  draftApiKey: string;
  draftUseFullUrl: boolean;
  draftBaseUrl: string;
  draftCustomUserAgent: string;
  draftRequestHeaders: Record<string, string>;
  localeText: LocaleText;
  setFetchingModels: (v: boolean) => void;
  setModelFetchError: (v: string | null) => void;
  setFetchedModelDetails: (v: ModelInfo[]) => void;
  setFetchedModels: (v: string[]) => void;
}

export async function performFetchModels(ctx: FetchModelsContext): Promise<void> {
  if (ctx.fetchingModels || !supportsModelFetch(ctx.draftTool)) return;
  if (ctx.draftProviderType === "github_copilot") {
    ctx.setFetchingModels(true);
    ctx.setModelFetchError(null);
    try {
      const models = await invoke<Array<{ id: string; name: string; vendor: string }>>("copilot_get_models", {
        accountId: ctx.draftOAuthAccountId.trim() || null,
      });
      const details = models.map((model) => ({
        id: model.id,
        display_name: model.name,
        context_window: null,
        max_output_tokens: null,
        input_price: null,
        output_price: null,
      }));
      ctx.setFetchedModelDetails(details);
      ctx.setFetchedModels(models.map((model) => model.id));
    } catch (error) {
      const message = formatModelFetchError(error, ctx.localeText);
      ctx.setModelFetchError(message);
      ctx.setFetchedModelDetails([]);
      ctx.setFetchedModels([]);
    } finally {
      ctx.setFetchingModels(false);
    }
    return;
  }
  if (ctx.draftProviderType === "codex_oauth") {
    ctx.setFetchingModels(true);
    ctx.setModelFetchError(null);
    try {
      const models = await invoke<Array<{ id: string; displayName?: string | null; ownedBy?: string | null }>>(
        "get_codex_oauth_models",
        { accountId: ctx.draftOAuthAccountId.trim() || null },
      );
      const details = models.map((model) => ({
        id: model.id,
        display_name: model.displayName ?? model.id,
        context_window: null,
        max_output_tokens: null,
        input_price: null,
        output_price: null,
      }));
      ctx.setFetchedModelDetails(details);
      ctx.setFetchedModels(models.map((model) => model.id));
    } catch (error) {
      const message = formatModelFetchError(error, ctx.localeText);
      ctx.setModelFetchError(message);
      ctx.setFetchedModelDetails([]);
      ctx.setFetchedModels([]);
    } finally {
      ctx.setFetchingModels(false);
    }
    return;
  }
  if (ctx.draftProviderType === "xai_oauth") {
    ctx.setFetchingModels(true);
    ctx.setModelFetchError(null);
    try {
      const models = await invoke<Array<{ id: string; displayName?: string | null; ownedBy?: string | null }>>(
        "get_xai_oauth_models",
        { accountId: ctx.draftOAuthAccountId.trim() || null },
      );
      const details = models.map((model) => ({
        id: model.id,
        display_name: model.displayName ?? model.id,
        context_window: null,
        max_output_tokens: null,
        input_price: null,
        output_price: null,
      }));
      ctx.setFetchedModelDetails(details);
      ctx.setFetchedModels(models.map((model) => model.id));
    } catch (error) {
      const message = formatModelFetchError(error, ctx.localeText);
      ctx.setModelFetchError(message);
      ctx.setFetchedModelDetails([]);
      ctx.setFetchedModels([]);
    } finally {
      ctx.setFetchingModels(false);
    }
    return;
  }
  if (!ctx.draftApiKey.trim()) {
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
  if (ctx.draftUseFullUrl && !ctx.draftBaseUrl.trim()) {
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

  ctx.setFetchingModels(true);
  ctx.setModelFetchError(null);
  try {
    const details = await invoke<ModelInfo[]>("fetch_provider_models_detailed", {
      toolId: ctx.draftTool,
      baseUrl: ctx.draftBaseUrl,
      apiKey: ctx.draftApiKey,
      useFullUrl: ctx.draftUseFullUrl,
      customUserAgent: ctx.draftCustomUserAgent,
      requestHeaders: ctx.draftRequestHeaders,
    });
    ctx.setFetchedModelDetails(details);
    const models = details.map((m) => m.id);
    ctx.setFetchedModels(models);
    if (models.length === 0) {
      showToast(
        "success",
        ctx.localeText(
          "已连接成功，但供应商没有返回可选模型",
          "Connected successfully, but the provider returned no models",
          "接続には成功しましたが、プロバイダーは利用可能なモデルを返しませんでした",
        ),
      );
    } else {
      showToast(
        "success",
        ctx.localeText(
          `已发现 ${models.length} 个模型`,
          `Discovered ${models.length} models`,
          `${models.length} 個のモデルを検出しました`,
        ),
      );
    }
  } catch (error) {
    const message = formatModelFetchError(error, ctx.localeText);
    ctx.setFetchedModels([]);
    ctx.setFetchedModelDetails([]);
    ctx.setModelFetchError(message);
    showToast("error", message);
  } finally {
    ctx.setFetchingModels(false);
  }
}

// 全局 cchub-shortcut-* 事件桥接到当前页面的 save / create / search 行为。
// 用 ref 包一层避免每次 deps 变化重新订阅事件。
interface KeyboardShortcutOptions {
  isEditing: boolean;
  canSave: boolean;
  onSave: () => Promise<void> | void;
  onCreate: () => Promise<void> | void;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
}

export function useProfilesKeyboardShortcuts(opts: KeyboardShortcutOptions): void {
  const flagsRef = useRef({ canSave: opts.canSave, isEditing: opts.isEditing });
  const handlersRef = useRef({ save: opts.onSave, create: opts.onCreate });

  useEffect(() => {
    flagsRef.current = { canSave: opts.canSave, isEditing: opts.isEditing };
  }, [opts.canSave, opts.isEditing]);

  useEffect(() => {
    handlersRef.current = { save: opts.onSave, create: opts.onCreate };
  }, [opts.onSave, opts.onCreate]);

  useEffect(() => {
    const handleSaveShortcut = () => {
      if (flagsRef.current.canSave) {
        void handlersRef.current.save();
      }
    };
    const handleNewShortcut = () => {
      if (!flagsRef.current.isEditing) {
        void handlersRef.current.create();
      }
    };
    const handleSearchShortcut = () => {
      if (flagsRef.current.isEditing) return;
      opts.searchInputRef.current?.focus();
      opts.searchInputRef.current?.select();
    };
    window.addEventListener("cchub-shortcut-save", handleSaveShortcut);
    window.addEventListener("cchub-shortcut-new", handleNewShortcut);
    window.addEventListener("cchub-shortcut-search", handleSearchShortcut);
    return () => {
      window.removeEventListener("cchub-shortcut-save", handleSaveShortcut);
      window.removeEventListener("cchub-shortcut-new", handleNewShortcut);
      window.removeEventListener("cchub-shortcut-search", handleSearchShortcut);
    };
  }, [opts.searchInputRef]);
}

export function useProfileCardText(locale: string) {
  return useMemo(
    () => ({
      activeTag: locale === "zh" ? "当前生效" : "Active",
      pingFast: locale === "zh" ? "快速" : "Fast",
      pingMedium: locale === "zh" ? "一般" : "Medium",
      pingSlow: locale === "zh" ? "较慢" : "Slow",
      pingError: locale === "zh" ? "异常" : "Error",
      streamHealthy: locale === "zh" ? "流检通过" : "Stream OK",
      streamReachable: locale === "zh" ? "流检可达" : "Stream Reachable",
      streamUnsupported: locale === "zh" ? "流检暂不支持" : "Stream Unsupported",
      streamUnconfigured: locale === "zh" ? "流检未配置" : "Stream Unconfigured",
      streamError: locale === "zh" ? "流检异常" : "Stream Error",
      dragEnabledTitle: locale === "zh" ? "拖拽调整顺序" : "Drag to reorder",
      dragDisabledTitle:
        locale === "zh" ? "先选择单个工具并清空搜索后再排序" : "Filter to one tool and clear search to reorder",
      pingTitle: locale === "zh" ? "端点测速" : "Ping endpoint",
      streamTitle: locale === "zh" ? "流式健康检查" : "Stream health check",
      usageTitle: locale === "zh" ? "查询用量" : "Query usage",
      duplicateTitle: locale === "zh" ? "复制" : "Duplicate",
      editTitle: locale === "zh" ? "编辑" : "Edit",
      deleteTitle: locale === "zh" ? "删除" : "Delete",
      activeButton: locale === "zh" ? "已生效" : "Active",
      applyButton: locale === "zh" ? "切换" : "Apply",
    }),
    [locale],
  );
}

export function useProfileDragHandlers(options: {
  reorderEnabled: boolean;
  draggingProfileId: string | null;
  setDraggingProfileId: (value: string | null) => void;
  setDragOverProfileId: (value: string | null) => void;
  reorderProfiles: (draggingId: string, targetId: string) => void | Promise<void>;
}) {
  const handleCardDragStart = (profileId: string) => options.setDraggingProfileId(profileId);
  const handleCardDragEnter = (profileId: string) => {
    if (options.reorderEnabled && options.draggingProfileId && options.draggingProfileId !== profileId) {
      options.setDragOverProfileId(profileId);
    }
  };
  const handleCardDragEnd = () => {
    options.setDraggingProfileId(null);
    options.setDragOverProfileId(null);
  };
  const handleCardDrop = (profileId: string) => {
    if (options.reorderEnabled && options.draggingProfileId) {
      void options.reorderProfiles(options.draggingProfileId, profileId);
    }
  };
  return { handleCardDragStart, handleCardDragEnter, handleCardDragEnd, handleCardDrop };
}

// 把编辑入口里大段的 setState 序列收到这里，Profiles.tsx 只保留薄包装。
export interface OpenEditModalContext {
  profile: ConfigProfile;
  profiles: ConfigProfile[];
  setEditingProfile: (v: ConfigProfile | null) => void;
  setShowCreateModal: (v: boolean) => void;
  setDraftName: (v: string) => void;
  setDraftTool: (v: string) => void;
  setDraftTargetTools: (v: string[]) => void;
  setDraftContent: (v: string) => void;
  setShowApiKey: (v: boolean) => void;
  setFetchingModels: (v: boolean) => void;
  setFetchedModels: (v: string[]) => void;
  setFetchedModelDetails: (v: ModelInfo[]) => void;
  setModelFetchError: (v: string | null) => void;
  setDraftFields: (v: StructuredDraftFields) => void;
  setDraftLoading: (v: boolean) => void;
  resetStructuredDraft: (toolId: string) => void;
}

export function performOpenEditModal(ctx: OpenEditModalContext): void {
  const { profile, profiles } = ctx;
  const sharedProfiles =
    profile.source_type === "shared" && profile.source_key
      ? profiles.filter((item) => item.source_type === "shared" && item.source_key === profile.source_key)
      : [profile];
  const otherProfiles = sharedProfiles.filter((item) => item.id !== profile.id);
  ctx.setEditingProfile(profile);
  ctx.setShowCreateModal(false);
  ctx.setDraftName(profile.name);
  ctx.setDraftTool(profile.tool_id);
  ctx.setDraftTargetTools(sharedProfiles.map((item) => item.tool_id));
  ctx.setDraftContent(prettyJson(profile.config_snapshot));
  ctx.setShowApiKey(false);
  ctx.setFetchingModels(false);
  ctx.setFetchedModels([]);
  ctx.setFetchedModelDetails([]);
  ctx.setModelFetchError(null);
  if (supportsStructuredConfig(profile.tool_id)) {
    let merged = createDefaultStructuredFields(profile.tool_id);
    for (const item of otherProfiles) {
      if (!supportsStructuredConfig(item.tool_id)) continue;
      merged = mergeSharedDraftFields(
        merged,
        item.tool_id,
        parseStructuredConfig(item.tool_id, item.config_snapshot),
        false,
      );
    }
    merged = mergeSharedDraftFields(
      merged,
      profile.tool_id,
      parseStructuredConfig(profile.tool_id, profile.config_snapshot),
      true,
    );
    ctx.setDraftFields(merged);
    ctx.setDraftContent(buildStructuredConfig(profile.tool_id, merged));
  } else {
    ctx.resetStructuredDraft("claude");
  }
  ctx.setDraftLoading(false);
}

// 关闭草稿编辑视图：所有 draft 状态归零
export interface CloseModalContext {
  setShowCreateModal: (v: boolean) => void;
  setEditingProfile: (v: ConfigProfile | null) => void;
  setDraftName: (v: string) => void;
  setDraftTargetTools: (v: string[]) => void;
  setDraftContent: (v: string) => void;
  setDraftLoading: (v: boolean) => void;
  setDraftFields: (v: StructuredDraftFields) => void;
  setDraftFragmentName: (v: string) => void;
  setSaving: (v: boolean) => void;
  setShowApiKey: (v: boolean) => void;
  setFetchingModels: (v: boolean) => void;
  setFetchedModels: (v: string[]) => void;
  setFetchedModelDetails: (v: ModelInfo[]) => void;
  setModelFetchError: (v: string | null) => void;
}

export function performCloseModal(ctx: CloseModalContext): void {
  ctx.setShowCreateModal(false);
  ctx.setEditingProfile(null);
  ctx.setDraftName("");
  ctx.setDraftTargetTools(["claude"]);
  ctx.setDraftContent("");
  ctx.setDraftLoading(false);
  ctx.setDraftFields(createDefaultStructuredFields("claude"));
  ctx.setDraftFragmentName("");
  ctx.setSaving(false);
  ctx.setShowApiKey(false);
  ctx.setFetchingModels(false);
  ctx.setFetchedModels([]);
  ctx.setFetchedModelDetails([]);
  ctx.setModelFetchError(null);
}

// search/filterTool 联合排序，把过滤+排序结果作为 stable identity 返回
export function useFilteredProfiles<
  T extends {
    id: string;
    name: string;
    tool_id: string;
    config_snapshot: string;
    sort_order: number;
    updated_at: string | null;
    created_at: string | null;
  },
>(profiles: T[], filterTool: string, deferredSearch: string, activeIdSet: Set<string>) {
  return useMemo(() => {
    const keyword = deferredSearch.trim().toLowerCase();
    return [...profiles]
      .filter((profile) => {
        if (filterTool && profile.tool_id !== filterTool) return false;
        if (!keyword) return true;
        return (
          profile.name.toLowerCase().includes(keyword) ||
          profile.tool_id.toLowerCase().includes(keyword) ||
          profile.config_snapshot.toLowerCase().includes(keyword)
        );
      })
      .sort((a, b) => {
        const toolDiff = a.tool_id.localeCompare(b.tool_id);
        if (!filterTool && toolDiff !== 0) return toolDiff;
        const orderDiff = (a.sort_order ?? 0) - (b.sort_order ?? 0);
        if (orderDiff !== 0) return orderDiff;
        const aTime = a.updated_at || a.created_at || "";
        const bTime = b.updated_at || b.created_at || "";
        const timeDiff = bTime.localeCompare(aTime);
        if (timeDiff !== 0) return timeDiff;
        const activeDiff = Number(activeIdSet.has(b.id)) - Number(activeIdSet.has(a.id));
        if (activeDiff !== 0) return activeDiff;
        return a.name.localeCompare(b.name);
      });
  }, [profiles, filterTool, deferredSearch, activeIdSet]);
}
