/* eslint-disable @typescript-eslint/no-explicit-any, react-hooks/exhaustive-deps, react-hooks/rules-of-hooks */
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getLocale } from "../lib/i18n";
import {
  applyPresetToFields,
  buildStructuredConfig,
  createDefaultStructuredFields,
  getPresetCategories,
  supportsStructuredConfig,
  type StructuredDraftFields,
} from "../lib/configProfiles";
import { showToast } from "../components/Toast";
import {
  useApplyConfigProfileMutation,
  useDeleteConfigProfileGroupAndRefreshMutation,
  useDeleteConfigProfileAndRefreshMutation,
  useDeleteProviderConfigFragmentMutation,
  useReorderConfigProfilesMutation,
  useSaveConfigProfileAndRefreshMutation,
  useSaveProviderConfigFragmentMutation,
  useSaveSharedConfigProfilesAndRefreshMutation,
  useUpdateConfigProfileAndRefreshMutation,
} from "../hooks/mutations";
import { type ModelInfo } from "../components/ModelSelector";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import { fetchProfilesPageData, queryKeys } from "../hooks/queries";

import {
  prettyJson,
  type ConfigProfile,
  type DetectedTool,
  type ProviderConfigFragment,
  type ProviderPingResult,
  type ProviderStreamCheckResult,
} from "./profiles/helpers";
import { mergeDraftFields, mergeSharedDraftFields, type DraftFieldsStateUpdate } from "./profiles/draftMerge";
import { buildEditorViewProps } from "./profiles/editorProps";
import {
  performCloseModal,
  performFetchModels,
  performOpenEditModal,
  useFilteredProfiles,
  useProfileCardText,
  useProfilesKeyboardShortcuts,
} from "./profiles/hooks";
import ProfilesConfirmDialogs from "./profiles/Dialogs";
import ProfileEditorView from "./profiles/EditorView";
import ProfilesListView from "./profiles/ListView";

export default function Profiles() {
  const queryClient = useQueryClient();
  const cachedProfilesPageData = queryClient.getQueryData<Awaited<ReturnType<typeof fetchProfilesPageData>>>(
    queryKeys.profilesPage,
  );
  const [profiles, setProfiles] = useState<ConfigProfile[]>(cachedProfilesPageData?.profiles ?? []);
  const [tools, setTools] = useState<DetectedTool[]>(cachedProfilesPageData?.tools ?? []);
  const [activeIds, setActiveIds] = useState<string[]>(cachedProfilesPageData?.activeIds ?? []);
  const [loading, setLoading] = useState(!cachedProfilesPageData);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [newTool, setNewTool] = useState(
    cachedProfilesPageData?.tools.find((tool) => tool.installed)?.id ??
      cachedProfilesPageData?.tools[0]?.id ??
      "claude",
  );
  const [saving, setSaving] = useState(false);
  const [applying, setApplying] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingProfile, setEditingProfile] = useState<ConfigProfile | null>(null);
  const [draftName, setDraftName] = useState("");
  const [draftTool, setDraftTool] = useState("claude");
  const [draftTargetTools, setDraftTargetTools] = useState<string[]>(["claude"]);
  const [draftContent, setDraftContent] = useState("");
  const [draftLoading, setDraftLoading] = useState(false);
  const [draftFields, setDraftFieldsState] = useState<StructuredDraftFields>(() =>
    createDefaultStructuredFields("claude"),
  );
  const [providerFragments, setProviderFragments] = useState<ProviderConfigFragment[]>(
    cachedProfilesPageData?.providerFragments ?? [],
  );
  const [draftFragmentName, setDraftFragmentName] = useState("");
  const [savingFragment, setSavingFragment] = useState(false);
  const [deletingFragmentId, setDeletingFragmentId] = useState<string | null>(null);
  const [confirmFragmentDelete, setConfirmFragmentDelete] = useState<ProviderConfigFragment | null>(null);
  const [filterTool, setFilterTool] = useState("claude");
  const [search, setSearch] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);
  const [draggingProfileId, setDraggingProfileId] = useState<string | null>(null);
  const [dragOverProfileId, setDragOverProfileId] = useState<string | null>(null);
  const [pingingId, setPingingId] = useState<string | null>(null);
  const [pingResults, setPingResults] = useState<Record<string, ProviderPingResult>>({});
  const [streamCheckingId, setStreamCheckingId] = useState<string | null>(null);
  const [streamCheckResults, setStreamCheckResults] = useState<Record<string, ProviderStreamCheckResult>>({});
  const [streamCheckConfirmProfile, setStreamCheckConfirmProfile] = useState<ConfigProfile | null>(null);
  const [fetchedModels, setFetchedModels] = useState<string[]>([]);
  const [fetchedModelDetails, setFetchedModelDetails] = useState<ModelInfo[]>([]);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [modelFetchError, setModelFetchError] = useState<string | null>(null);

  const { baseUrl: draftBaseUrl, useFullUrl: draftUseFullUrl, apiKey: draftApiKey } = draftFields;

  const [confirmAction, setConfirmAction] = useState<{ type: string; profile: ConfigProfile } | null>(null);
  const locale = getLocale();
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const draftFieldsRef = useRef(draftFields);
  const applyConfigProfileMutation = useApplyConfigProfileMutation();
  const saveConfigProfileMutation = useSaveConfigProfileAndRefreshMutation();
  const updateConfigProfileMutation = useUpdateConfigProfileAndRefreshMutation();
  const saveSharedConfigProfilesMutation = useSaveSharedConfigProfilesAndRefreshMutation();
  const deleteConfigProfileMutation = useDeleteConfigProfileAndRefreshMutation();
  const deleteConfigProfileGroupMutation = useDeleteConfigProfileGroupAndRefreshMutation();
  const reorderConfigProfilesMutation = useReorderConfigProfilesMutation();
  const saveProviderConfigFragmentMutation = useSaveProviderConfigFragmentMutation<ProviderConfigFragment>();
  const deleteProviderConfigFragmentMutation = useDeleteProviderConfigFragmentMutation();
  const localeText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );
  const installedTools = useMemo(() => tools.filter((tool) => tool.installed), [tools]);

  useEffect(() => {
    draftFieldsRef.current = draftFields;
  }, [draftFields]);

  const applyProfilesPageData = useCallback((data: Awaited<ReturnType<typeof fetchProfilesPageData>>) => {
    setProfiles(data.profiles);
    setTools(data.tools);
    setActiveIds(data.activeIds);
    setProviderFragments(data.providerFragments);
    setNewTool((prev) => {
      const installed = data.tools.filter((tool) => tool.installed);
      if (installed.some((tool) => tool.id === prev)) return prev;
      return installed[0]?.id || data.tools[0]?.id || "claude";
    });
  }, []);

  const load = useCallback(
    async (options: { force?: boolean } = {}) => {
      const { force = false } = options;
      if (!queryClient.getQueryData(queryKeys.profilesPage)) {
        setLoading(true);
      }
      setLoadError(null);
      try {
        const data = await queryClient.fetchQuery({
          queryKey: queryKeys.profilesPage,
          queryFn: fetchProfilesPageData,
          staleTime: force ? 0 : 30_000,
        });
        applyProfilesPageData(data);
      } catch (e) {
        console.error(e);
        setLoadError(String(e));
        showToast("error", locale === "zh" ? `加载失败: ${e}` : `Load failed: ${e}`);
      } finally {
        setLoading(false);
      }
    },
    [applyProfilesPageData, locale, queryClient],
  );

  const updateDraftFieldsState = useCallback((next: DraftFieldsStateUpdate) => {
    setDraftFieldsState((current) => {
      const resolved = typeof next === "function" ? next(current) : next;
      draftFieldsRef.current = resolved;
      return resolved;
    });
  }, []);

  const setDraftFields = useCallback(
    (fields: StructuredDraftFields) => {
      updateDraftFieldsState(fields);
    },
    [updateDraftFieldsState],
  );

  const buildCurrentFields = useCallback((next: Partial<StructuredDraftFields> = {}): StructuredDraftFields => {
    return mergeDraftFields(draftFieldsRef.current, next);
  }, []);

  const updateStructuredDraft = useCallback(
    (toolId: string, next: Partial<StructuredDraftFields>) => {
      const fields = buildCurrentFields(next);
      setDraftFields(fields);
      setDraftContent(buildStructuredConfig(toolId, fields));
    },
    [buildCurrentFields],
  );

  function sortProviderFragments(fragments: ProviderConfigFragment[]) {
    return [...fragments].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt) || a.name.localeCompare(b.name));
  }

  function normalizeFragmentFields(fragment: ProviderConfigFragment): StructuredDraftFields {
    return {
      ...createDefaultStructuredFields(draftTool),
      ...(fragment.fields || {}),
    };
  }

  const resetStructuredDraft = useCallback(
    (toolId: string) => {
      const defaults = createDefaultStructuredFields(toolId);
      setDraftFields(defaults);
      setDraftContent(buildStructuredConfig(toolId, defaults));
      setDraftLoading(false);
    },
    [setDraftFields],
  );

  const openCreateModal = useCallback(
    async (toolId?: string) => {
      const selectedTool = toolId || newTool;
      if (!installedTools.length) {
        showToast("error", locale === "zh" ? "没有可用工具配置" : "No available tool configuration");
        return;
      }
      setEditingProfile(null);
      setDraftName("");
      setDraftTool(selectedTool);
      setDraftTargetTools([selectedTool]);
      setDraftContent("");
      setShowCreateModal(true);
      setSaving(false);
      setNewTool(selectedTool);
      setShowApiKey(false);
      setFetchingModels(false);
      setFetchedModels([]);
      setFetchedModelDetails([]);
      setModelFetchError(null);
      updateDraftFieldsState((current) => ({ ...current, apiFormat: "anthropic" }));
      if (supportsStructuredConfig(selectedTool)) {
        resetStructuredDraft(selectedTool);
        return;
      }
      setDraftContent("");
      setDraftLoading(true);
      try {
        const configContent = await invoke<string>("read_tool_config", { toolId: selectedTool });
        setDraftContent(prettyJson(configContent));
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? `读取配置失败: ${e}` : `Failed to read configuration: ${e}`);
      } finally {
        setDraftLoading(false);
      }
    },
    [installedTools.length, locale, newTool, resetStructuredDraft, updateDraftFieldsState],
  );

  const openEditModal = useCallback(
    (profile: ConfigProfile) => {
      performOpenEditModal({
        profile,
        profiles,
        setEditingProfile,
        setShowCreateModal,
        setDraftName,
        setDraftTool,
        setDraftTargetTools,
        setDraftContent,
        setShowApiKey,
        setFetchingModels,
        setFetchedModels,
        setFetchedModelDetails,
        setModelFetchError,
        setDraftFields,
        setDraftLoading,
        resetStructuredDraft,
      });
    },
    [profiles, resetStructuredDraft, setDraftFields],
  );

  const closeModal = useCallback(() => {
    performCloseModal({
      setShowCreateModal,
      setEditingProfile,
      setDraftName,
      setDraftTargetTools,
      setDraftContent,
      setDraftLoading,
      setDraftFields,
      setDraftFragmentName,
      setSaving,
      setShowApiKey,
      setFetchingModels,
      setFetchedModels,
      setFetchedModelDetails,
      setModelFetchError,
    });
  }, [setDraftFields]);

  const doDeleteFragment = useCallback(
    async (fragment: ProviderConfigFragment) => {
      setDeletingFragmentId(fragment.id);
      try {
        await deleteProviderConfigFragmentMutation.mutateAsync({ id: fragment.id });
        setProviderFragments((current) => current.filter((item) => item.id !== fragment.id));
        showToast(
          "success",
          localeText("配置片段已删除", "Provider fragment deleted", "Provider フラグメントを削除しました"),
        );
      } catch (e) {
        console.error(e);
        showToast(
          "error",
          localeText(`删除片段失败: ${e}`, `Failed to delete fragment: ${e}`, `フラグメントの削除に失敗しました: ${e}`),
        );
      } finally {
        setDeletingFragmentId((current) => (current === fragment.id ? null : current));
      }
    },
    [deleteProviderConfigFragmentMutation, localeText],
  );

  const handleSaveModal = useCallback(async () => {
    if (!draftName.trim() || saving) return;
    setSaving(true);
    try {
      if (
        supportsStructuredConfig(draftTool) &&
        (draftTargetTools.length > 1 || editingProfile?.source_type === "shared")
      ) {
        const targetTools = draftTargetTools.filter((toolId) => supportsStructuredConfig(toolId));
        const profilesPayload = targetTools.map((toolId) => ({
          toolId,
          configSnapshot: buildStructuredConfig(toolId, buildCurrentFields()),
        }));
        const { data } = await saveSharedConfigProfilesMutation.mutateAsync({
          name: draftName.trim(),
          profiles: profilesPayload,
          groupKey: editingProfile?.source_type === "shared" ? (editingProfile.source_key ?? null) : null,
          replaceProfileId: editingProfile && editingProfile.source_type !== "shared" ? editingProfile.id : null,
        });
        applyProfilesPageData(data);
        showToast("success", localeText("共享配置已保存", "Shared provider saved", "共有 Provider を保存しました"));
      } else if (editingProfile) {
        const data = await updateConfigProfileMutation.mutateAsync({
          id: editingProfile.id,
          name: draftName.trim(),
          configSnapshot: draftContent,
        });
        applyProfilesPageData(data);
        showToast("success", locale === "zh" ? "配置已更新" : "Configuration updated");
      } else {
        const { data } = await saveConfigProfileMutation.mutateAsync({
          name: draftName.trim(),
          toolId: draftTool,
          configSnapshot: draftContent,
        });
        applyProfilesPageData(data);
        showToast("success", locale === "zh" ? "配置已保存" : "Configuration saved");
      }
      closeModal();
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `保存失败: ${e}` : `Save failed: ${e}`);
    } finally {
      setSaving(false);
    }
  }, [
    buildCurrentFields,
    applyProfilesPageData,
    closeModal,
    draftContent,
    draftName,
    draftTargetTools,
    draftTool,
    editingProfile,
    locale,
    localeText,
    saving,
    saveConfigProfileMutation,
    saveSharedConfigProfilesMutation,
    updateConfigProfileMutation,
  ]);

  useProfilesKeyboardShortcuts({
    canSave: (showCreateModal || !!editingProfile) && draftName.trim().length > 0 && !saving,
    isEditing: showCreateModal || !!editingProfile,
    onSave: handleSaveModal,
    onCreate: openCreateModal,
    searchInputRef,
  });

  const doApply = useCallback(
    async (profile: ConfigProfile) => {
      setApplying(profile.id);
      try {
        const result = await applyConfigProfileMutation.mutateAsync(profile.id);
        setActiveIds(result.activeProfileIds);
        setProfiles((current) =>
          current.map((item) => (item.id === profile.id ? { ...item, updated_at: result.appliedAt } : item)),
        );
        showToast("success", locale === "zh" ? "配置已切换" : "Configuration switched");
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? `切换失败: ${e}` : `Switch failed: ${e}`);
      } finally {
        setApplying(null);
      }
    },
    [applyConfigProfileMutation, locale],
  );

  const handleDelete = useCallback(async (profile: ConfigProfile) => {
    setConfirmAction({ type: "delete", profile });
  }, []);

  const doDelete = useCallback(
    async (profile: ConfigProfile) => {
      try {
        if (profile.source_type === "shared" && profile.source_key) {
          const { removedCount, data } = await deleteConfigProfileGroupMutation.mutateAsync({
            sourceKey: profile.source_key,
          });
          applyProfilesPageData(data);
          showToast(
            "success",
            localeText(
              `共享配置组已删除（${removedCount} 个 App）`,
              `Shared provider group deleted (${removedCount} apps)`,
              `共有 Provider グループを削除しました（${removedCount} 件の App）`,
            ),
          );
          return;
        }
        if (profile.source_type !== "manual") {
          showToast(
            "error",
            locale === "zh" ? "当前配置/同步配置不支持删除" : "Live or synced profiles cannot be deleted",
          );
          return;
        }
        const data = await deleteConfigProfileMutation.mutateAsync({ id: profile.id });
        applyProfilesPageData(data);
        showToast("success", locale === "zh" ? "配置已删除" : "Configuration deleted");
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? `删除失败: ${e}` : `Delete failed: ${e}`);
      }
    },
    [applyProfilesPageData, deleteConfigProfileGroupMutation, deleteConfigProfileMutation, locale, localeText],
  );

  const handleDuplicate = useCallback(
    async (profile: ConfigProfile) => {
      try {
        const name = profile.name + (locale === "zh" ? " (副本)" : " (Copy)");
        const { data } = await saveConfigProfileMutation.mutateAsync({
          name,
          toolId: profile.tool_id,
          configSnapshot: profile.config_snapshot,
        });
        applyProfilesPageData(data);
        showToast("success", locale === "zh" ? "配置已复制" : "Configuration duplicated");
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? `复制失败: ${e}` : `Duplicate failed: ${e}`);
      }
    },
    [applyProfilesPageData, locale, saveConfigProfileMutation],
  );

  const handlePing = useCallback(
    async (profile: ConfigProfile) => {
      setPingingId(profile.id);
      try {
        const result = await invoke<ProviderPingResult>("ping_provider_endpoint", { id: profile.id });
        setPingResults((current) => ({ ...current, [profile.id]: result }));
        if (result.status !== "error") {
          showToast(
            "success",
            locale === "zh"
              ? `已测速 ${profile.name}${result.latency_ms != null ? ` · ${result.latency_ms}ms` : ""}`
              : `Pinged ${profile.name}${result.latency_ms != null ? ` · ${result.latency_ms}ms` : ""}`,
          );
        } else {
          showToast("error", result.message);
        }
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? `测速失败: ${e}` : `Ping failed: ${e}`);
      } finally {
        setPingingId((current) => (current === profile.id ? null : current));
      }
    },
    [locale],
  );

  const runStreamCheck = useCallback(
    async (profile: ConfigProfile) => {
      setStreamCheckingId(profile.id);
      try {
        const result = await invoke<ProviderStreamCheckResult>("stream_check_config_profile", { id: profile.id });
        setStreamCheckResults((current) => ({ ...current, [profile.id]: result }));
        if (result.status === "healthy" || result.status === "reachable") {
          showToast(
            "success",
            locale === "zh"
              ? `流式检查完成：${profile.name}${result.latency_ms != null ? ` · ${result.latency_ms}ms` : ""}`
              : `Stream check finished: ${profile.name}${result.latency_ms != null ? ` · ${result.latency_ms}ms` : ""}`,
          );
        } else {
          showToast("error", result.message);
        }
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? `流式检查失败: ${e}` : `Stream check failed: ${e}`);
      } finally {
        setStreamCheckingId((current) => (current === profile.id ? null : current));
      }
    },
    [locale],
  );

  const handleStreamCheck = useCallback(
    (profile: ConfigProfile) => {
      if (localStorage.getItem("cchub-stream-check-confirmed") === "1") {
        void runStreamCheck(profile);
        return;
      }
      setStreamCheckConfirmProfile(profile);
    },
    [runStreamCheck],
  );

  const reorderProfiles = useCallback(
    async (sourceId: string, targetId: string) => {
      if (!filterTool || sourceId === targetId || search.trim()) return;
      const orderedProfiles = [...profiles]
        .filter((profile) => profile.tool_id === filterTool)
        .sort((a, b) => {
          const orderDiff = (a.sort_order ?? 0) - (b.sort_order ?? 0);
          if (orderDiff !== 0) return orderDiff;
          const aTime = a.updated_at || a.created_at || "";
          const bTime = b.updated_at || b.created_at || "";
          return bTime.localeCompare(aTime);
        });
      const fromIndex = orderedProfiles.findIndex((profile) => profile.id === sourceId);
      const toIndex = orderedProfiles.findIndex((profile) => profile.id === targetId);
      if (fromIndex < 0 || toIndex < 0 || fromIndex === toIndex) return;

      const nextOrdered = [...orderedProfiles];
      const [moved] = nextOrdered.splice(fromIndex, 1);
      nextOrdered.splice(toIndex, 0, moved);
      const nextOrderMap = new Map(nextOrdered.map((profile, index) => [profile.id, index]));

      setProfiles((current) =>
        current.map((profile) =>
          profile.tool_id === filterTool && nextOrderMap.has(profile.id)
            ? { ...profile, sort_order: nextOrderMap.get(profile.id) ?? profile.sort_order }
            : profile,
        ),
      );

      try {
        await reorderConfigProfilesMutation.mutateAsync({
          toolId: filterTool,
          orderedIds: nextOrdered.map((profile) => profile.id),
        });
      } catch (e) {
        console.error(e);
        showToast("error", locale === "zh" ? `排序失败: ${e}` : `Reorder failed: ${e}`);
        await load({ force: true });
      } finally {
        setDraggingProfileId(null);
        setDragOverProfileId(null);
      }
    },
    [filterTool, load, locale, profiles, reorderConfigProfilesMutation, search],
  );

  useEffect(() => {
    void load();
  }, [load]);

  const activeIdSet = useMemo(() => new Set(activeIds), [activeIds]);
  const presetCategories = useMemo(() => getPresetCategories(draftTool), [draftTool]);
  const reorderEnabled = Boolean(filterTool) && search.trim().length === 0;
  const structuredInstalledTools = useMemo(
    () => tools.filter((tool) => tool.installed && supportsStructuredConfig(tool.id)),
    [tools],
  );
  const toolNameMap = useMemo(() => Object.fromEntries(tools.map((tool) => [tool.id, tool.name])), [tools]);
  const handleToggleDraftTargetTool = useCallback(
    (toolId: string) => {
      if (!supportsStructuredConfig(toolId)) return;
      const alreadySelected = draftTargetTools.includes(toolId);
      if (alreadySelected && draftTargetTools.length === 1) {
        return;
      }

      const structuredToolIds = structuredInstalledTools.map((tool) => tool.id);
      const nextTargets = structuredToolIds.filter((id) => {
        if (id === toolId) return !alreadySelected;
        return draftTargetTools.includes(id);
      });

      if (nextTargets.length === 0) {
        return;
      }

      setDraftTargetTools(nextTargets);
      if (!nextTargets.includes(draftTool)) {
        setDraftTool(nextTargets[0]);
        setDraftContent(buildStructuredConfig(nextTargets[0], buildCurrentFields()));
      }
    },
    [buildCurrentFields, draftTargetTools, draftTool, structuredInstalledTools],
  );
  const handleSaveFragment = useCallback(async () => {
    if (!supportsStructuredConfig(draftTool) || savingFragment || !draftFragmentName.trim()) return;
    setSavingFragment(true);
    try {
      const saved = await saveProviderConfigFragmentMutation.mutateAsync({
        name: draftFragmentName.trim(),
        targetTools: draftTargetTools.filter((toolId) => supportsStructuredConfig(toolId)),
        fields: buildCurrentFields(),
      });
      setProviderFragments((current) =>
        sortProviderFragments([saved, ...current.filter((fragment) => fragment.id !== saved.id)]),
      );
      setDraftFragmentName("");
      showToast(
        "success",
        localeText("配置片段已保存", "Provider fragment saved", "Provider フラグメントを保存しました"),
      );
    } catch (e) {
      console.error(e);
      showToast(
        "error",
        localeText(`保存片段失败: ${e}`, `Failed to save fragment: ${e}`, `フラグメントの保存に失敗しました: ${e}`),
      );
    } finally {
      setSavingFragment(false);
    }
  }, [
    buildCurrentFields,
    draftFragmentName,
    draftTargetTools,
    draftTool,
    localeText,
    saveProviderConfigFragmentMutation,
    savingFragment,
  ]);
  const handleApplyFragmentById = useCallback(
    (fragmentId: string) => {
      const fragment = providerFragments.find((item) => item.id === fragmentId);
      if (!fragment) return;
      const includeToolSpecific = fragment.targetTools.includes(draftTool);
      const merged = mergeSharedDraftFields(
        buildCurrentFields(),
        draftTool,
        normalizeFragmentFields(fragment),
        true,
        includeToolSpecific,
      );
      setDraftFields(merged);
      setDraftContent(buildStructuredConfig(draftTool, merged));
      showToast(
        "success",
        localeText("已应用配置片段", "Provider fragment applied", "Provider フラグメントを適用しました"),
      );
    },
    [buildCurrentFields, draftTool, localeText, providerFragments],
  );
  const handleApplyPreset = useCallback(
    (presetId: string) => {
      const preset = presetCategories.flatMap((group) => group.presets).find((item) => item.id === presetId);
      if (!preset) return;
      const next = applyPresetToFields(draftTool, preset.id, buildCurrentFields());
      updateStructuredDraft(draftTool, next);
    },
    [buildCurrentFields, draftTool, presetCategories, updateStructuredDraft],
  );
  const handleSelectDraftOauthAccount = useCallback(
    (accountId: string | null) => {
      updateStructuredDraft(draftTool, { oauthAccountId: accountId || "" });
    },
    [draftTool, updateStructuredDraft],
  );
  const handleFetchModels = useCallback(
    () =>
      performFetchModels({
        fetchingModels,
        draftTool,
        draftApiKey,
        draftUseFullUrl,
        draftBaseUrl,
        localeText,
        setFetchingModels,
        setModelFetchError,
        setFetchedModelDetails,
        setFetchedModels,
      }),
    [draftApiKey, draftBaseUrl, draftTool, draftUseFullUrl, fetchingModels, localeText],
  );
  const handleRequestFragmentDelete = useCallback(
    (fragmentId: string) => {
      const fragment = providerFragments.find((item) => item.id === fragmentId);
      if (!fragment) return;
      setConfirmFragmentDelete(fragment);
    },
    [providerFragments],
  );
  const handleRefreshProfiles = useCallback(() => {
    void load({ force: true });
  }, [load]);
  const handleOpenCreateProfile = useCallback(() => {
    void openCreateModal();
  }, [openCreateModal]);
  const handleSearchChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setSearch(event.target.value);
  }, []);
  const handleClearSearch = useCallback(() => {
    setSearch("");
  }, []);
  const handleDraftNameChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setDraftName(event.target.value);
  }, []);
  const handleDraftFragmentNameChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setDraftFragmentName(event.target.value);
  }, []);
  const handleToggleShowApiKey = useCallback(() => {
    setShowApiKey((current) => !current);
  }, []);
  const handleToggleFilterTool = useCallback((toolId: string) => {
    setFilterTool((prev) => (prev === toolId ? "" : toolId));
  }, []);
  const handleSaveFragmentClick = useCallback(() => {
    void handleSaveFragment();
  }, [handleSaveFragment]);
  const handleDraftToolChange = useCallback(
    async (event: ChangeEvent<HTMLSelectElement>) => {
      const toolId = event.target.value;
      setDraftTool(toolId);
      setNewTool(toolId);
      setFetchingModels(false);
      setFetchedModels([]);
      setFetchedModelDetails([]);
      setModelFetchError(null);
      updateDraftFieldsState((current) => ({ ...current, apiFormat: "anthropic" }));
      if (supportsStructuredConfig(toolId)) {
        if (draftTargetTools.length > 1 || editingProfile?.source_type === "shared") {
          if (!draftTargetTools.includes(toolId)) {
            setDraftTargetTools((current) => [...current, toolId]);
          }
          setDraftContent(buildStructuredConfig(toolId, buildCurrentFields()));
        } else {
          resetStructuredDraft(toolId);
          setDraftTargetTools([toolId]);
        }
      } else {
        setDraftContent("");
        setDraftLoading(true);
        try {
          const configContent = await invoke<string>("read_tool_config", { toolId });
          setDraftContent(prettyJson(configContent));
        } catch (error) {
          console.error(error);
        } finally {
          setDraftLoading(false);
        }
      }
    },
    [buildCurrentFields, draftTargetTools, editingProfile?.source_type, resetStructuredDraft, updateDraftFieldsState],
  );
  const profileCardText = useProfileCardText(locale);

  const toolCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const profile of profiles) {
      counts[profile.tool_id] = (counts[profile.tool_id] || 0) + 1;
    }
    return counts;
  }, [profiles]);

  const sharedGroupCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const profile of profiles) {
      if (profile.source_type === "shared" && profile.source_key) {
        counts[profile.source_key] = (counts[profile.source_key] || 0) + 1;
      }
    }
    return counts;
  }, [profiles]);

  // search 输入触发的 filter 涉及对每个 profile 的 config_snapshot（可能 KB 级 JSON）
  // 做 toLowerCase + includes，搜索框敲键阶段交给 deferredValue 跑在低优先级。
  const deferredSearch = useDeferredValue(search);
  const filteredProfiles = useFilteredProfiles(profiles, filterTool, deferredSearch, activeIdSet);

  function handleCardDragStart(profileId: string) {
    setDraggingProfileId(profileId);
  }

  function handleCardDragEnter(profileId: string) {
    if (reorderEnabled && draggingProfileId && draggingProfileId !== profileId) {
      setDragOverProfileId(profileId);
    }
  }

  function handleCardDragEnd() {
    setDraggingProfileId(null);
    setDragOverProfileId(null);
  }

  function handleCardDrop(profileId: string) {
    if (!reorderEnabled || !draggingProfileId) return;
    void reorderProfiles(draggingProfileId, profileId);
  }

  if (loading) {
    return <LoadingState label={localeText("加载中...", "Loading...", "読み込み中...")} />;
  }

  if (loadError) {
    return (
      <ErrorState
        title={localeText("配置加载失败", "Failed to load profiles", "設定の読み込みに失敗しました")}
        message={loadError}
        retryLabel={localeText("刷新", "Refresh", "再読み込み")}
        onRetry={() => {
          void load({ force: true });
        }}
      />
    );
  }

  const isEditing = showCreateModal || !!editingProfile;
  const isStructured = supportsStructuredConfig(draftTool);

  if (isEditing) {
    const editorProps = buildEditorViewProps({
      locale,
      localeText,
      editingProfile,
      closeModal,
      handleSaveModal,
      draftName,
      saving,
      tools,
      draftTool,
      isStructured,
      draftTargetTools,
      structuredInstalledTools,
      handleDraftToolChange,
      handleDraftNameChange,
      handleToggleDraftTargetTool,
      presetCategories,
      draftFragmentName,
      savingFragment,
      providerFragments,
      toolNameMap,
      deletingFragmentId,
      handleApplyPreset,
      handleDraftFragmentNameChange,
      handleSaveFragmentClick,
      handleApplyFragmentById,
      handleRequestFragmentDelete,
      draftFields,
      showApiKey,
      updateStructuredDraft,
      handleSelectDraftOauthAccount,
      handleToggleShowApiKey,
      fetchedModels,
      fetchedModelDetails,
      fetchingModels,
      modelFetchError,
      handleFetchModels,
      draftContent,
      draftLoading,
      setDraftContent,
    });
    return <ProfileEditorView {...editorProps} />;
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <ProfilesListView
        locale={locale}
        localeText={localeText}
        profiles={profiles}
        activeIds={activeIds}
        tools={tools}
        installedTools={installedTools}
        toolCounts={toolCounts}
        filterTool={filterTool}
        filteredProfiles={filteredProfiles}
        activeIdSet={activeIdSet}
        pingResults={pingResults}
        streamCheckResults={streamCheckResults}
        sharedGroupCounts={sharedGroupCounts}
        search={search}
        searchInputRef={searchInputRef}
        reorderEnabled={reorderEnabled}
        draggingProfileId={draggingProfileId}
        dragOverProfileId={dragOverProfileId}
        pingingId={pingingId}
        streamCheckingId={streamCheckingId}
        applying={applying}
        profileCardText={profileCardText}
        handleRefreshProfiles={handleRefreshProfiles}
        handleOpenCreateProfile={handleOpenCreateProfile}
        handleSearchChange={handleSearchChange}
        handleClearSearch={handleClearSearch}
        handleToggleFilterTool={handleToggleFilterTool}
        handleCardDragStart={handleCardDragStart}
        handleCardDragEnter={handleCardDragEnter}
        handleCardDragEnd={handleCardDragEnd}
        handleCardDrop={handleCardDrop}
        handlePing={handlePing}
        handleStreamCheck={handleStreamCheck}
        doApply={doApply}
        handleDuplicate={handleDuplicate}
        openEditModal={openEditModal}
        handleDelete={handleDelete}
      />

      <ProfilesConfirmDialogs
        locale={locale}
        localeText={localeText}
        confirmAction={confirmAction}
        setConfirmAction={setConfirmAction}
        doDelete={doDelete}
        sharedGroupCounts={sharedGroupCounts}
        confirmFragmentDelete={confirmFragmentDelete}
        setConfirmFragmentDelete={setConfirmFragmentDelete}
        doDeleteFragment={doDeleteFragment}
        streamCheckConfirmProfile={streamCheckConfirmProfile}
        setStreamCheckConfirmProfile={setStreamCheckConfirmProfile}
        runStreamCheck={runStreamCheck}
      />
    </div>
  );
}
