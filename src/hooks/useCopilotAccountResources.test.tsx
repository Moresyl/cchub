import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCopilotAccountResources } from "./useCopilotAccountResources";
import type { CopilotAccountResources, CopilotAuthStatus, GitHubAccount } from "../lib/copilotAccounts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
const one: GitHubAccount = { id: "1", login: "one", revision: "login-one", avatar_url: null, authenticated_at: 1 };
const two: GitHubAccount = { ...one, id: "2", login: "two", revision: "login-two" };
function status(accounts = [one, two], defaultId: string | null = "1"): CopilotAuthStatus {
  return {
    accounts,
    default_account_id: defaultId,
    authenticated: accounts.length > 0,
    username: accounts[0]?.login ?? null,
    expires_at: null,
  };
}
function resources(account = one): CopilotAccountResources {
  return {
    account,
    fetched_at: "2026-10-01T08:00:00Z",
    usage: null,
    models: [],
    usage_error: "unavailable",
    models_error: null,
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset().mockResolvedValue(vi.fn());
});
afterEach(cleanup);

describe("owned Copilot resource requests", () => {
  it.each([false, true])(
    "ignores a late result or error after selecting another account (failure=%s)",
    async (failure) => {
      const old = deferred<CopilotAccountResources>();
      vi.mocked(invoke)
        .mockResolvedValueOnce(status())
        .mockReturnValueOnce(old.promise)
        .mockResolvedValueOnce(resources(two));
      const hook = renderHook(useCopilotAccountResources);
      await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
      expect(invoke).toHaveBeenNthCalledWith(2, "copilot_get_account_resources", {
        accountId: "1",
        expectedRevision: "login-one",
      });
      act(() => hook.result.current.select("2"));
      await waitFor(() => expect(hook.result.current.data?.account.id).toBe("2"));
      await act(async () => (failure ? old.reject(new Error("secret token")) : old.resolve(resources())));
      expect(hook.result.current.data?.account.id).toBe("2");
      expect(hook.result.current.error).toBe(false);
      expect(hook.result.current.loading).toBe(false);
    },
  );

  it("preserves an explicit selection when the default changes and clears removed selections", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(status())
      .mockResolvedValueOnce(resources())
      .mockResolvedValueOnce(resources(two));
    const hook = renderHook(useCopilotAccountResources);
    await waitFor(() => expect(hook.result.current.data).not.toBeNull());
    act(() => hook.result.current.select("2"));
    await waitFor(() => expect(hook.result.current.data?.account.id).toBe("2"));
    vi.mocked(invoke)
      .mockResolvedValueOnce(status([one, two], "2"))
      .mockResolvedValueOnce(resources(two));
    await act(async () => {
      await hook.result.current.refresh();
    });
    expect(hook.result.current.selection).toBe("2");
    vi.mocked(invoke)
      .mockResolvedValueOnce(status([one]))
      .mockResolvedValueOnce(resources());
    await act(async () => {
      await hook.result.current.refresh();
    });
    expect(hook.result.current.selection).toBe("");
    expect(hook.result.current.data?.account.id).toBe("1");
  });

  it("invalidates an old login immediately on auth events and queries the new revision", async () => {
    const old = deferred<CopilotAccountResources>();
    const nextAuth = deferred<CopilotAuthStatus>();
    const replacement = { ...one, revision: "new-login", login: "new-one" };
    vi.mocked(invoke)
      .mockResolvedValueOnce(status())
      .mockReturnValueOnce(old.promise)
      .mockReturnValueOnce(nextAuth.promise)
      .mockResolvedValueOnce(resources(replacement));
    const hook = renderHook(useCopilotAccountResources);
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
    const event = vi.mocked(listen).mock.calls[0][1];
    act(() => event({ event: "copilot-auth-changed", id: 1, payload: null }));
    await act(async () => old.resolve(resources()));
    expect(hook.result.current.data).toBeNull();
    expect(hook.result.current.loading).toBe(true);
    await act(async () => nextAuth.resolve(status([replacement, two])));
    await waitFor(() => expect(hook.result.current.data?.account.revision).toBe("new-login"));
    expect(invoke).toHaveBeenLastCalledWith("copilot_get_account_resources", {
      accountId: "1",
      expectedRevision: "new-login",
    });
  });

  it("clears displayed resources after logout without querying a fallback account", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(status()).mockResolvedValueOnce(resources());
    const hook = renderHook(useCopilotAccountResources);
    await waitFor(() => expect(hook.result.current.data).not.toBeNull());
    vi.mocked(invoke).mockResolvedValueOnce(status([], null));
    await act(async () => {
      await hook.result.current.refresh();
    });
    expect(hook.result.current.data).toBeNull();
    expect(hook.result.current.account).toBeNull();
    expect(hook.result.current.loading).toBe(false);
    expect(invoke).toHaveBeenCalledTimes(3);
  });

  it("ignores a late account list after a newer explicit selection", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(status()).mockResolvedValueOnce(resources());
    const hook = renderHook(useCopilotAccountResources);
    await waitFor(() => expect(hook.result.current.data).not.toBeNull());
    const old = deferred<CopilotAuthStatus>();
    vi.mocked(invoke).mockReturnValueOnce(old.promise).mockResolvedValueOnce(resources(two));
    let pending!: Promise<void>;
    act(() => {
      pending = hook.result.current.refresh();
      hook.result.current.select("2");
    });
    await waitFor(() => expect(hook.result.current.data?.account.id).toBe("2"));
    await act(async () => {
      old.resolve(status([one]));
      await pending;
    });
    expect(hook.result.current.account?.id).toBe("2");
    expect(hook.result.current.selection).toBe("2");
  });

  it("rejects mismatched revisions and allows a sanitized retry", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(status())
      .mockResolvedValueOnce(resources({ ...one, revision: "old-login" }));
    const hook = renderHook(useCopilotAccountResources);
    await waitFor(() => expect(hook.result.current.error).toBe(true));
    expect(hook.result.current.data).toBeNull();
    vi.mocked(invoke).mockResolvedValueOnce(status()).mockResolvedValueOnce(resources());
    await act(async () => {
      await hook.result.current.refresh();
    });
    expect(hook.result.current.error).toBe(false);
    expect(hook.result.current.data?.account.revision).toBe(one.revision);
  });

  it("disposes late event registration and falls back to manual refresh when events fail", async () => {
    const registration = deferred<() => void>();
    const stop = vi.fn();
    vi.mocked(listen).mockReturnValueOnce(registration.promise);
    const first = renderHook(useCopilotAccountResources);
    first.unmount();
    await act(async () => registration.resolve(stop));
    expect(stop).toHaveBeenCalledOnce();
    expect(invoke).not.toHaveBeenCalled();
    vi.mocked(listen).mockRejectedValueOnce(new Error("no events"));
    vi.mocked(invoke).mockResolvedValueOnce(status()).mockResolvedValueOnce(resources());
    const next = renderHook(useCopilotAccountResources);
    await waitFor(() => expect(next.result.current.data).not.toBeNull());
  });
});
