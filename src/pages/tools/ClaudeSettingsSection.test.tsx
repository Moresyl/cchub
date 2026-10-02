import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import ClaudeSettingsSection from "./ClaudeSettingsSection";
import { useClaudeSettings, claudeSettingsKey, type ClaudeSettings } from "./useClaudeSettings";

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
const initial: ClaudeSettings = {
  permission_mode: "normal",
  allow_count: 3,
  ask_count: 2,
  deny_count: 1,
  auto_update: "stable",
  model: "gateway/custom-opus-model",
  tool_search: "auto:5",
  legacy_tool_search: "true",
  config_revision: "a".repeat(64),
};
function setup() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, refetchOnMount: false }, mutations: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, wrapper };
}
function tab() {
  const context = setup();
  return { ...context, ...render(<ClaudeSettingsSection uiText={(zh) => zh} />, { wrapper: context.wrapper }) };
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

describe("confirmed Claude user settings", () => {
  it("keeps exact custom values, rule counts and legacy migration visible until explicitly changed", async () => {
    mockInvoke.mockResolvedValueOnce(initial);
    tab();
    expect((await screen.findByRole("combobox", { name: "权限模式" })).textContent).toBe("normal");
    expect(screen.getByRole("combobox", { name: "默认模型" }).textContent).toContain("gateway/custom-opus-model");
    expect(screen.getByRole("combobox", { name: "Tool Search" }).textContent).toBe("auto:5");
    expect(screen.getByText("已有规则：允许 3 · 询问 2 · 拒绝 1")).toBeTruthy();
    expect(screen.getByText(/发现旧版 settings.local.json 值：true/)).toBeTruthy();
    expect(mockInvoke).toHaveBeenCalledExactlyOnceWith("get_claude_settings");
    expect(screen.queryByRole("slider")).toBeNull();
    expect(screen.getByText(/发现旧版权限值 normal/)).toBeTruthy();
  });

  it("uses native permissions and retains confirmed values while waiting for acknowledgement", async () => {
    const pending = deferred<ClaudeSettings>();
    mockInvoke.mockResolvedValueOnce(initial).mockReturnValueOnce(pending.promise);
    tab();
    const trigger = await screen.findByRole("combobox", { name: "权限模式" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("option", { name: "规划 · 批准后执行" }));
    await screen.findByText("正在保存…");
    expect(trigger.textContent).toBe("normal");
    expect(trigger.hasAttribute("disabled")).toBe(true);
    expect(mockInvoke).toHaveBeenLastCalledWith("set_claude_setting", {
      key: "permission_mode",
      value: "plan",
      expectedRevision: initial.config_revision,
    });
    await act(async () => pending.resolve({ ...initial, permission_mode: "plan", config_revision: "b".repeat(64) }));
    await waitFor(() => expect(trigger.textContent).toBe("规划 · 批准后执行"));
  });

  it("masks failed reads and can recover without substituting fabricated defaults", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("keep-secret")).mockResolvedValueOnce(initial);
    tab();
    await screen.findByText("无法读取 Claude 设置");
    expect(screen.queryByRole("combobox")).toBeNull();
    expect(screen.queryByText(/keep-secret/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
    await screen.findByRole("combobox", { name: "权限模式" });
  });

  it("preserves update settings on failed write and requires reload before another edit", async () => {
    mockInvoke.mockResolvedValueOnce(initial).mockRejectedValueOnce("keep-secret").mockResolvedValueOnce(initial);
    tab();
    const trigger = await screen.findByRole("combobox", { name: "自动更新" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("option", { name: "关闭后台自动更新" }));
    await screen.findByText("设置未保存");
    expect(trigger.textContent).toBe("稳定频道");
    expect(trigger.hasAttribute("disabled")).toBe(true);
    expect(screen.queryByText(/keep-secret/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
    await waitFor(() => expect(trigger.hasAttribute("disabled")).toBe(false));
  });

  it("does not migrate legacy tool-search UI until native confirmation", async () => {
    mockInvoke
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce({ ...initial, tool_search: "", legacy_tool_search: "", config_revision: "b".repeat(64) });
    tab();
    const trigger = await screen.findByRole("combobox", { name: "Tool Search" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("option", { name: "使用客户端默认值" }));
    await waitFor(() => expect(trigger.textContent).toBe("使用客户端默认值"));
    expect(mockInvoke).toHaveBeenLastCalledWith("set_claude_setting", {
      key: "tool_search",
      value: "",
      expectedRevision: initial.config_revision,
    });
    expect(screen.queryByText(/发现旧版 settings.local.json/)).toBeNull();
  });

  it("rejects malformed native snapshots instead of making them editable", async () => {
    mockInvoke.mockResolvedValueOnce({ ...initial, deny_count: -1 });
    tab();
    await screen.findByText("无法读取 Claude 设置");
    expect(screen.queryByRole("combobox")).toBeNull();
  });

  it("validates a tool-search threshold and only submits it on explicit application", async () => {
    mockInvoke.mockResolvedValueOnce(initial).mockResolvedValueOnce({
      ...initial,
      tool_search: "auto:7",
      legacy_tool_search: "",
      config_revision: "b".repeat(64),
    });
    tab();
    const input = await screen.findByRole("spinbutton", { name: "工具定义占上下文的阈值（%）" });
    const apply = screen.getByRole("button", { name: "应用阈值" });
    expect((input as HTMLInputElement).value).toBe("5");
    expect(apply.hasAttribute("disabled")).toBe(true);
    fireEvent.change(input, { target: { value: "101" } });
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(apply.hasAttribute("disabled")).toBe(true);
    fireEvent.change(input, { target: { value: "7" } });
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    fireEvent.click(apply);
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Tool Search" }).textContent).toBe("auto:7"));
    expect(mockInvoke).toHaveBeenLastCalledWith("set_claude_setting", {
      key: "tool_search",
      value: "auto:7",
      expectedRevision: initial.config_revision,
    });
    expect(screen.getByRole("button", { name: "应用阈值" }).hasAttribute("disabled")).toBe(true);
  });

  it("serializes actual writes and accepts only the acknowledged settings snapshot", async () => {
    const context = setup();
    const pending = deferred<ClaudeSettings>();
    mockInvoke.mockResolvedValueOnce(initial).mockReturnValueOnce(pending.promise);
    const { result } = renderHook(useClaudeSettings, { wrapper: context.wrapper });
    await waitFor(() => expect(result.current.settings).toEqual(initial));
    let first!: Promise<boolean>;
    await act(async () => {
      first = result.current.save("model", "opus");
      expect(await result.current.save("permission_mode", "bypassPermissions")).toBe(false);
    });
    expect(result.current.settings?.model).toBe(initial.model);
    expect(mockInvoke).toHaveBeenCalledTimes(2);
    await act(async () => {
      pending.resolve({ ...initial, model: "opus", config_revision: "b".repeat(64) });
      expect(await first).toBe(true);
    });
    expect(context.client.getQueryData<ClaudeSettings>(claudeSettingsKey)?.model).toBe("opus");
  });
});
