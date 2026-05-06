import { useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { FolderSearch } from "lucide-react";
import { memo, useCallback, useState } from "react";
import { queryKeys } from "../hooks/queries";
import { useSetHermesRootOverrideMutation } from "../hooks/mutations";
import { showToast } from "./Toast";

function DashboardHermesRootOverrideComponent() {
  const queryClient = useQueryClient();
  const [saving, setSaving] = useState(false);
  const setHermesRootOverrideMutation = useSetHermesRootOverrideMutation();

  const handleClick = useCallback(async () => {
    setSaving(true);
    try {
      const current = await invoke<string | null>("get_hermes_root_override");
      const next = window.prompt(
        "Hermes root override\n留空则恢复默认 ~/.hermes\nExample: \\\\wsl.localhost\\Ubuntu\\home\\user\\.hermes",
        current || "",
      );
      if (next === null) return;
      await setHermesRootOverrideMutation.mutateAsync({
        value: next.trim() ? next.trim() : null,
      });
      await queryClient.invalidateQueries({ queryKey: queryKeys.detectTools });
      showToast("success", "Hermes root updated");
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setSaving(false);
    }
  }, [queryClient, setHermesRootOverrideMutation]);

  return (
    <button
      type="button"
      className="btn btn-ghost btn-xs"
      onClick={() => void handleClick()}
      disabled={saving}
      style={{ gap: 4 }}
      title="Override Hermes root"
    >
      {saving ? <div className="spinner" style={{ width: 10, height: 10 }} /> : <FolderSearch size={12} />}
      Root
    </button>
  );
}

export default memo(DashboardHermesRootOverrideComponent);
