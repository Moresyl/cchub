import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAsyncResource } from "../../lib/asyncState";
import { showToast } from "../../components/Toast";
import { t } from "../../lib/i18n";

export default function useConfigDraft<T>(command: string, normalize: (data: unknown) => T) {
  const resource = useAsyncResource(async () => normalize(await invoke<unknown>(command)));
  const [draft, setDraft] = useState<T | null>(null);
  const [saved, setSaved] = useState<T | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const writing = useRef(false);
  useEffect(() => {
    if (!resource.data) return;
    setDraft(resource.data);
    setSaved(resource.data);
    setSaveFailed(false);
  }, [resource.data]);
  async function save(value: T, persist: (value: T) => Promise<unknown>, blocked: boolean) {
    if (!draft || resource.loading || resource.error || writing.current || blocked) return;
    writing.current = true;
    setSaving(true);
    setSaveFailed(false);
    try {
      await persist(value);
      setSaved(value);
      setDraft(value);
      showToast("success", t().openClaw.saveSuccess);
    } catch {
      setSaveFailed(true);
      showToast("error", t().openClaw.saveFailed);
    } finally {
      writing.current = false;
      setSaving(false);
    }
  }
  return {
    ...resource,
    loading: resource.loading || (!draft && !resource.error),
    draft,
    saving,
    saveFailed,
    dirty: JSON.stringify(draft) !== JSON.stringify(saved),
    update: (value: T) => {
      if (!writing.current) setDraft(value);
    },
    reset: () => {
      if (!writing.current) {
        setDraft(saved);
        setSaveFailed(false);
      }
    },
    save,
  };
}
