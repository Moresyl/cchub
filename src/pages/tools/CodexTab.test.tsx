import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import CodexTab from "./CodexTab";
import { codexSettingsKey, useCodexSettings, type CodexSettings } from "./useCodexSettings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);
const originalScroll = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollIntoView");
beforeAll(() => Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() }));
afterAll(() => {
  if (originalScroll) Object.defineProperty(HTMLElement.prototype, "scrollIntoView", originalScroll);
  else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
});
beforeEach(() => mockInvoke.mockReset());
afterEach(cleanup);

const initial: CodexSettings = {
  approval_mode: "workspace-write",
  approval_policy: "on-request",
  sandbox_mode: "workspace-write",
  permission_profile: "",
  reasoning_effort: "future-effort",
  disable_response_storage: false,
  context_window_1m: false,
  context_window: 512_000,
  legacy_personality: false,
  profile_selected: false,
  config_revision: "a".repeat(64),
};
function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, wrapper };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function tab() {
  const context = setup();
  return { ...context, ...render(<CodexTab uiText={(zh) => zh} />, { wrapper: context.wrapper }) };
}

describe("confirmed native Codex settings", () => {
  it("preserves unknown reasoning and submits the model default with the loaded revision", async () => {
    mockInvoke
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce({ ...initial, reasoning_effort: "", config_revision: "b".repeat(64) });
    tab();
    const trigger = await screen.findByRole("combobox", { name: "推理强度" });
    expect(trigger.textContent).toBe("future-effort");
    expect(mockInvoke).toHaveBeenCalledExactlyOnceWith("get_codex_settings");
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("option", { name: "使用模型默认值" }));
    await waitFor(() => expect(trigger.textContent).toBe("使用模型默认值"));
    expect(mockInvoke).toHaveBeenLastCalledWith("set_codex_setting", {
      key: "reasoning_effort",
      value: "",
      expectedRevision: initial.config_revision,
    });
  });

  it("renders a reloadable masked error without inventing default settings", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("token = keep-secret")).mockResolvedValueOnce(initial);
    tab();
    await screen.findByText("无法读取 Codex 设置");
    expect(screen.queryByRole("combobox")).toBeNull();
    expect(screen.queryByText(/keep-secret/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
    await screen.findByRole("combobox", { name: "权限模式" });
  });

  it("keeps the confirmed value on write failure and requires reload before further edits", async () => {
    mockInvoke
      .mockResolvedValueOnce(initial)
      .mockRejectedValueOnce(new Error("keep-secret"))
      .mockResolvedValueOnce(initial);
    tab();
    const toggle = await screen.findByRole("switch", { name: "1M 上下文上限" });
    fireEvent.click(toggle);
    await screen.findByText("设置未保存");
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    expect(toggle.hasAttribute("disabled")).toBe(true);
    expect(screen.queryByText(/keep-secret/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
    await waitFor(() => expect(toggle.hasAttribute("disabled")).toBe(false));
    expect(screen.queryByText("设置未保存")).toBeNull();
    expect(screen.getByText("当前上限：512,000")).toBeTruthy();
  });

  it("displays custom permissions and selected configuration profiles without changing them", async () => {
    mockInvoke.mockResolvedValueOnce({ ...initial, approval_mode: "custom", profile_selected: true });
    tab();
    const trigger = await screen.findByRole("combobox", { name: "权限模式" });
    expect(trigger.textContent).toBe("保留自定义权限");
    expect(trigger.hasAttribute("disabled")).toBe(true);
    expect(mockInvoke).toHaveBeenCalledExactlyOnceWith("get_codex_settings");
  });

  it("rejects malformed response contracts instead of crashing or enabling controls", async () => {
    mockInvoke.mockResolvedValueOnce({ ...initial, config_revision: undefined });
    tab();
    await screen.findByText("无法读取 Codex 设置");
    expect(screen.queryByRole("switch")).toBeNull();
  });

  it("does not display cached values after a refresh failure", async () => {
    const context = setup();
    context.client.setQueryData(codexSettingsKey, initial);
    mockInvoke.mockRejectedValueOnce(new Error("unreadable"));
    render(<CodexTab uiText={(zh) => zh} />, { wrapper: context.wrapper });
    await screen.findByText("无法读取 Codex 设置");
    expect(screen.queryByRole("combobox")).toBeNull();
    expect(mockInvoke.mock.calls.map(([command]) => command)).toEqual(["get_codex_settings"]);
  });
});

describe("Codex settings save ordering", () => {
  it("blocks duplicate writes, exposes pending state and only adopts the native acknowledgement", async () => {
    const response = deferred<CodexSettings>();
    mockInvoke.mockResolvedValueOnce(initial).mockImplementationOnce(() => response.promise);
    const context = setup();
    const hook = renderHook(useCodexSettings, { wrapper: context.wrapper });
    await waitFor(() => expect(hook.result.current.settings).toEqual(initial));
    let pending!: Promise<boolean>;
    await act(async () => {
      pending = hook.result.current.save("context_window_1m", "true");
      expect(await hook.result.current.save("context_window_1m", "false")).toBe(false);
    });
    expect(hook.result.current.saving).toBe(true);
    expect(hook.result.current.settings?.context_window_1m).toBe(false);
    expect(mockInvoke).toHaveBeenCalledTimes(2);
    const accepted = {
      ...initial,
      context_window: 1_000_000,
      context_window_1m: true,
      config_revision: "b".repeat(64),
    };
    await act(async () => {
      response.resolve(accepted);
      expect(await pending).toBe(true);
    });
    await waitFor(() => expect(hook.result.current.settings).toEqual(accepted));
    expect(hook.result.current.saving).toBe(false);
  });

  it("does not let a save acknowledgement from the old location replace a newer reload", async () => {
    const response = deferred<CodexSettings>();
    let current = initial;
    mockInvoke.mockImplementation(async (cmd) => (cmd === "get_codex_settings" ? current : response.promise));
    const context = setup();
    const hook = renderHook(useCodexSettings, { wrapper: context.wrapper });
    await waitFor(() => expect(hook.result.current.settings).toEqual(initial));
    let pending!: Promise<boolean>;
    await act(async () => {
      pending = hook.result.current.save("reasoning_effort", "high");
    });
    const newer = { ...initial, reasoning_effort: "low", config_revision: "c".repeat(64) };
    current = newer;
    await act(async () => {
      await context.client.invalidateQueries({ queryKey: codexSettingsKey });
    });
    await waitFor(() => expect(hook.result.current.settings).toEqual(newer));
    await act(async () => {
      response.resolve({ ...initial, reasoning_effort: "high", config_revision: "b".repeat(64) });
      await pending;
    });
    await waitFor(() => expect(hook.result.current.settings).toEqual(newer));
  });

  it("does not install an old acknowledgement after unmounting", async () => {
    const response = deferred<CodexSettings>();
    mockInvoke.mockResolvedValueOnce(initial).mockImplementationOnce(() => response.promise);
    const context = setup();
    const hook = renderHook(useCodexSettings, { wrapper: context.wrapper });
    await waitFor(() => expect(hook.result.current.settings).toEqual(initial));
    let pending!: Promise<boolean>;
    await act(async () => {
      pending = hook.result.current.save("reasoning_effort", "high");
    });
    hook.unmount();
    await act(async () => {
      response.resolve({ ...initial, reasoning_effort: "high", config_revision: "b".repeat(64) });
      await pending;
    });
    expect(context.client.getQueryData(codexSettingsKey)).toEqual(initial);
    expect(context.client.getQueryState(codexSettingsKey)?.isInvalidated).toBe(true);
  });
});
