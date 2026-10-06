import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { rememberDraft, readDraft } from "./draft";
import { samePreferences, validConfig, type SyncConfig, type SyncResult, type SyncState } from "./types";

type Failure = "read" | "save" | "sync" | "conflict" | null;
interface View {
  state: SyncState | null;
  baseline: SyncConfig | null;
  draft: SyncConfig | null;
  loading: boolean;
  busy: "save" | "sync" | null;
  failure: Failure;
  result: SyncResult | null;
}
function checkedState(value: SyncState): SyncState {
  if (!value || !validConfig(value.config) || typeof value.configPath !== "string") throw new Error("Invalid settings");
  return value;
}
export function usePricingSettings() {
  const [view, setView] = useState<View>({
    state: null,
    baseline: null,
    draft: null,
    loading: true,
    busy: null,
    failure: null,
    result: null,
  });
  const current = useRef(view);
  const owner = useRef({ mounted: false, request: 0, reading: false, writing: false });
  const commit = useCallback((next: View) => {
    current.current = next;
    if (next.baseline && next.draft) rememberDraft({ baseline: next.baseline, draft: next.draft });
    setView(next);
  }, []);
  const refresh = useCallback(
    async (discardDraft = false) => {
      const active = owner.current;
      if (!active.mounted || active.writing || active.reading) return;
      active.reading = true;
      const request = ++active.request;
      commit({ ...current.current, loading: true });
      try {
        const state = checkedState(await invoke<SyncState>("get_models_dev_sync_config"));
        if (!active.mounted || active.request !== request) return;
        const prior = current.current;
        const pending = prior.baseline && prior.draft ? { baseline: prior.baseline, draft: prior.draft } : readDraft();
        const dirty = !discardDraft && !!pending && !samePreferences(pending.baseline, pending.draft);
        const conflict = dirty && !samePreferences(pending.baseline, state.config);
        commit({
          ...prior,
          state,
          baseline: dirty ? pending.baseline : state.config,
          draft: dirty ? pending.draft : state.config,
          loading: false,
          failure: conflict ? "conflict" : null,
        });
      } catch {
        if (active.mounted && active.request === request)
          commit({ ...current.current, loading: false, failure: "read" });
      } finally {
        if (active.request === request) active.reading = false;
      }
    },
    [commit],
  );
  useEffect(() => {
    const active = owner.current;
    active.mounted = true;
    void refresh();
    return () => {
      active.mounted = false;
      active.reading = false;
      active.writing = false;
      ++active.request;
    };
  }, [refresh]);
  const update = useCallback(
    (change: (draft: SyncConfig) => SyncConfig) => {
      if (!owner.current.mounted || owner.current.writing || owner.current.reading || !current.current.draft) return;
      const prior = current.current;
      commit({
        ...prior,
        draft: change(prior.draft!),
        result: null,
        failure: prior.failure === "save" || prior.failure === "sync" ? null : prior.failure,
      });
    },
    [commit],
  );
  const run = useCallback(
    async (sync: boolean) => {
      const active = owner.current;
      const prior = current.current;
      if (
        !active.mounted ||
        active.writing ||
        prior.loading ||
        !prior.baseline ||
        !prior.draft ||
        prior.failure === "conflict" ||
        prior.failure === "read"
      )
        return;
      active.writing = true;
      const request = ++active.request;
      commit({ ...prior, busy: sync ? "sync" : "save", failure: null, result: null });
      let saved = prior.baseline;
      let step: Failure = "save";
      try {
        if (!samePreferences(prior.baseline, prior.draft)) {
          const next = checkedState(
            await invoke<SyncState>("save_models_dev_sync_config", {
              config: prior.draft,
              expectedConfig: prior.baseline,
            }),
          );
          if (!active.mounted || active.request !== request) return;
          saved = next.config;
          commit({ ...current.current, state: next, baseline: saved, draft: saved });
        }
        if (sync) {
          step = "sync";
          const result = await invoke<SyncResult>("sync_models_dev_pricing", { force: true, expectedConfig: saved });
          if (!active.mounted || active.request !== request) return;
          // The command's successful result is authoritative even if a later status read fails.
          const state = {
            ...current.current.state!,
            config: { ...saved, lastSyncAt: result.syncedAt, lastSyncError: null },
          };
          commit({ ...current.current, state, baseline: state.config, draft: state.config, result });
        }
      } catch (error) {
        if (active.mounted && active.request === request) {
          const conflict = error === "PRICING_SETTINGS_CONFLICT";
          commit({ ...current.current, failure: conflict ? "conflict" : step });
        }
      } finally {
        if (active.mounted && active.request === request) {
          active.writing = false;
          commit({ ...current.current, busy: null });
        }
      }
    },
    [commit],
  );
  const dirty = !!view.baseline && !!view.draft && !samePreferences(view.baseline, view.draft);
  const blocked = !!view.busy || view.loading || view.failure === "conflict" || view.failure === "read";
  return { ...view, dirty, blocked, refresh, update, save: () => run(false), sync: () => run(true) };
}
