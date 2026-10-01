import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CopilotAccountResources, CopilotAuthStatus, GitHubAccount } from "../lib/copilotAccounts";

function selectedAccount(auth: CopilotAuthStatus | null, selection: string) {
  return (
    auth?.accounts.find((account) => account.id === (selection || auth.default_account_id)) ?? auth?.accounts[0] ?? null
  );
}

export function useCopilotAccountResources() {
  const [auth, setAuth] = useState<CopilotAuthStatus | null>(null);
  const [selection, setSelection] = useState("");
  const [data, setData] = useState<CopilotAccountResources | null>(null);
  const [loading, setLoading] = useState(true);
  const [listing, setListing] = useState(true);
  const [error, setError] = useState(false);
  const active = useRef({ mounted: false, generation: 0, selection: "", auth: null as CopilotAuthStatus | null });

  const query = useCallback(async (account: GitHubAccount | null, generation: number) => {
    const owner = active.current;
    if (!account) {
      if (owner.mounted && owner.generation === generation) setLoading(false);
      return;
    }
    try {
      const result = await invoke<CopilotAccountResources>("copilot_get_account_resources", {
        accountId: account.id,
        expectedRevision: account.revision,
      });
      if (!owner.mounted || owner.generation !== generation) return;
      if (
        result?.account?.id !== account.id ||
        result.account.revision !== account.revision ||
        (result.models !== null && !Array.isArray(result.models))
      )
        throw new Error("Stale Copilot resources");
      setData(result);
    } catch {
      if (owner.mounted && owner.generation === generation) {
        setData(null);
        setError(true);
      }
    } finally {
      if (owner.mounted && owner.generation === generation) setLoading(false);
    }
  }, []);

  const refresh = useCallback(async () => {
    const owner = active.current;
    const generation = ++owner.generation;
    setData(null);
    setError(false);
    setLoading(true);
    setListing(true);
    try {
      const next = await invoke<CopilotAuthStatus>("copilot_get_auth_status");
      if (!owner.mounted || owner.generation !== generation) return;
      if (!Array.isArray(next?.accounts) || next.accounts.some((account) => !account.id || !account.revision)) {
        throw new Error("Invalid Copilot accounts");
      }
      if (owner.selection && !next.accounts.some((account) => account.id === owner.selection)) {
        owner.selection = "";
        setSelection("");
      }
      owner.auth = next;
      setAuth(next);
      setListing(false);
      await query(selectedAccount(next, owner.selection), generation);
    } catch {
      if (owner.mounted && owner.generation === generation) {
        owner.auth = null;
        setAuth(null);
        setError(true);
        setLoading(false);
        setListing(false);
      }
    }
  }, [query]);

  const select = useCallback(
    (id: string) => {
      const owner = active.current;
      if (id && !owner.auth?.accounts.some((account) => account.id === id)) return;
      const generation = ++owner.generation;
      owner.selection = id;
      setSelection(id);
      setData(null);
      setError(false);
      setListing(false);
      setLoading(true);
      void query(selectedAccount(owner.auth, id), generation);
    },
    [query],
  );

  useEffect(() => {
    const owner = active.current;
    owner.mounted = true;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    // Subscribe before the snapshot so an authentication change is not lost.
    void listen("copilot-auth-changed", () => {
      void refresh();
    })
      .then((stop) => {
        if (disposed) stop();
        else {
          unlisten = stop;
          void refresh();
        }
      })
      .catch(() => {
        if (!disposed) void refresh();
      });
    return () => {
      disposed = true;
      owner.mounted = false;
      ++owner.generation;
      unlisten?.();
    };
  }, [refresh]);

  return { auth, selection, account: selectedAccount(auth, selection), data, loading, listing, error, refresh, select };
}
