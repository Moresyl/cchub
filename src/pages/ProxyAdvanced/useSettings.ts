import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSaveProxyAdvancedConfigMutation } from "../../hooks/mutations";
import type { OptimizerConfig, ProxyAdvancedSettings, RectifierConfig } from "./types";

export default function useSettings() {
  const [config, setConfig] = useState<OptimizerConfig | null>(null);
  const [rectConfig, setRectConfig] = useState<RectifierConfig | null>(null);
  const [revision, setRevision] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [saveError, setSaveError] = useState(false);
  const [saving, setSaving] = useState(false);
  const generation = useRef(0);
  const mounted = useRef(false);
  const writing = useRef(false);
  const mutation = useSaveProxyAdvancedConfigMutation();

  const load = useCallback(async () => {
    if (writing.current) return;
    const attempt = ++generation.current;
    setLoading(true);
    setLoadError(false);
    try {
      const data = await invoke<ProxyAdvancedSettings>("get_proxy_advanced_config");
      if (!mounted.current || attempt !== generation.current) return;
      setConfig(data.config);
      setRectConfig(data.rectifierConfig);
      setRevision(data.revision);
      setSaveError(false);
    } catch {
      if (mounted.current && attempt === generation.current) setLoadError(true);
    } finally {
      if (mounted.current && attempt === generation.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void load();
    return () => {
      mounted.current = false;
      generation.current += 1;
    };
  }, [load]);

  async function save() {
    if (writing.current || loading || loadError || saveError || !config || !rectConfig || !revision) return false;
    writing.current = true;
    setSaving(true);
    try {
      const nextRevision = await mutation.mutateAsync({
        config,
        rectifierConfig: rectConfig,
        expectedRevision: revision,
      });
      if (mounted.current) setRevision(nextRevision);
      return true;
    } catch (error) {
      if (mounted.current) setSaveError(true);
      throw error;
    } finally {
      writing.current = false;
      if (mounted.current) setSaving(false);
    }
  }

  return { config, setConfig, rectConfig, setRectConfig, loading, loadError, saveError, saving, load, save };
}
