import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { StrictMode, useState } from "react";
import ProxyAdvanced from "./ProxyAdvanced";
import { setLocale } from "../lib/i18n";

const { invoke, save } = vi.hoisted(() => ({ invoke: vi.fn(), save: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../components/CircuitBreakerPanel", () => ({ default: () => null }));
vi.mock("../components/FailoverQueueManager", () => ({ default: () => null }));
vi.mock("../components/ProviderRoutingPanel", () => ({
  default: function MockRoutingPanel({ appType }: { appType?: string }) {
    const [initialTool] = useState(appType ?? "all");
    return <span data-testid="routing-tool-instance">{initialTool}</span>;
  },
}));
vi.mock("../hooks/mutations", () => ({ useSaveProxyAdvancedConfigMutation: () => ({ mutateAsync: save }) }));

function config(enabled = true) {
  return {
    enabled,
    thinkingOptimizer: false,
    cacheInjection: true,
    cacheTtl: "5m",
    bodyFilter: true,
    bodyFilterWhitelist: ["_keep"],
    modelMapper: true,
    modelMapperDefault: "fixture-model",
    modelMapperRules: [],
    copilotOptimizer: false,
    copilotModelNormalization: true,
    codexFieldStripping: false,
    circuitFailureThreshold: 3,
    circuitSuccessThreshold: 2,
    circuitTimeoutSecs: 60,
    failoverEnabled: true,
    maxProfileRetries: 3,
    streamingFirstByteTimeout: 60,
    streamingIdleTimeout: 120,
    nonStreamingTimeout: 600,
  };
}

function settings(enabled = true, revision = "revision-1") {
  return {
    config: config(enabled),
    rectifierConfig: { enabled: true, thinkingSignature: true, thinkingBudget: true },
    revision,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  setLocale("zh");
  vi.clearAllMocks();
  invoke.mockResolvedValue(settings());
  save.mockResolvedValue("revision-2");
});
afterEach(cleanup);

describe("ProxyAdvanced settings", () => {
  it("creates a fresh routing editor when the host switches tool modes", async () => {
    const view = render(<ProxyAdvanced mode="claude" />);
    await screen.findByRole("spinbutton", { name: "普通响应总超时 (秒)" });
    expect(screen.getByTestId("routing-tool-instance").textContent).toBe("claude");
    view.rerender(<ProxyAdvanced mode="codex" />);
    expect(screen.getByTestId("routing-tool-instance").textContent).toBe("codex");
    view.rerender(<ProxyAdvanced mode="all" />);
    expect(screen.getByTestId("routing-tool-instance").textContent).toBe("all");
  });

  it.each(["all", "claude", "codex"] as const)("provides and saves request deadlines in %s mode", async (mode) => {
    render(<ProxyAdvanced mode={mode} />);
    const input = await screen.findByRole("spinbutton", { name: "普通响应总超时 (秒)" });
    await waitFor(() => expect((input as HTMLInputElement).value).toBe("600"));
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.change(input, { target: { value: "0" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({
          config: expect.objectContaining({
            nonStreamingTimeout: 0,
            streamingFirstByteTimeout: 60,
            streamingIdleTimeout: 120,
          }),
          expectedRevision: "revision-1",
        }),
      ),
    );
    expect(input.getAttribute("data-control-size")).toBe("md");
  });

  it("disables optimizer descendants for keyboard and pointer users when the master switch is off", async () => {
    invoke.mockResolvedValue(settings(false));
    render(<ProxyAdvanced />);
    const input = await screen.findByRole("textbox", { name: "缓存 TTL" });
    expect(input.matches(":disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "添加白名单字段" }).matches(":disabled")).toBe(true);
    expect((screen.getByRole("spinbutton", { name: "普通响应总超时 (秒)" }) as HTMLInputElement).disabled).toBe(false);
  });

  it("adds and removes whitelist fields with labeled shared controls", async () => {
    render(<ProxyAdvanced />);
    const input = await screen.findByRole("textbox", { name: "白名单（保留的 _ 字段）" });
    fireEvent.change(input, { target: { value: "_extra" } });
    fireEvent.click(screen.getByRole("button", { name: "添加白名单字段" }));
    fireEvent.click(screen.getByRole("button", { name: "删除 _keep" }));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({
          config: expect.objectContaining({ bodyFilterWhitelist: ["_extra"] }),
        }),
      ),
    );
  });

  it("shows a safe read error instead of fabricated editable defaults, then retries", async () => {
    invoke.mockRejectedValueOnce(new Error("fixture secret: never display this"));
    render(<ProxyAdvanced />);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("无法读取代理设置");
    expect(alert.textContent).not.toContain("fixture secret");
    expect(screen.queryByRole("spinbutton")).toBeNull();
    expect(screen.queryByRole("button", { name: "保存" })).toBeNull();
    expect(save).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "重新读取设置" }));
    await screen.findByRole("spinbutton", { name: "普通响应总超时 (秒)" });
    expect(invoke).toHaveBeenLastCalledWith("get_proxy_advanced_config");
  });

  it("ignores a stale initial read after StrictMode starts a newer read", async () => {
    const old = deferred<ReturnType<typeof settings>>();
    invoke.mockReturnValueOnce(old.promise);
    render(
      <StrictMode>
        <ProxyAdvanced />
      </StrictMode>,
    );
    const input = await screen.findByRole("spinbutton", { name: "普通响应总超时 (秒)" });
    old.reject(new Error("late read failure"));
    await waitFor(() => expect((input as HTMLInputElement).value).toBe("600"));
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  });

  it("freezes editing and prevents duplicate writes until the save finishes, then uses the returned revision", async () => {
    const pending = deferred<string>();
    save.mockReturnValueOnce(pending.promise);
    render(<ProxyAdvanced />);
    const input = await screen.findByRole("spinbutton", { name: "普通响应总超时 (秒)" });
    const button = screen.getByRole("button", { name: "保存" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(input.matches(":disabled")).toBe(true);
    expect(save).toHaveBeenCalledTimes(1);
    pending.resolve("confirmed-2");
    await waitFor(() => expect(button.matches(":disabled")).toBe(false));
    fireEvent.click(button);
    await waitFor(() =>
      expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ expectedRevision: "confirmed-2" })),
    );
  });

  it("retains failed-save drafts and disables saving until a confirmed reload", async () => {
    save.mockRejectedValueOnce(new Error("conflict: private payload"));
    render(<ProxyAdvanced />);
    const input = await screen.findByRole("spinbutton", { name: "普通响应总超时 (秒)" });
    fireEvent.change(input, { target: { value: "42" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("未保存的修改仍保留");
    expect(alert.textContent).not.toContain("private payload");
    expect((input as HTMLInputElement).value).toBe("42");
    expect(input.matches(":disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "保存" }).matches(":disabled")).toBe(true);
    invoke.mockRejectedValueOnce(new Error("read still unavailable"));
    fireEvent.click(screen.getByRole("button", { name: "重新读取设置" }));
    await screen.findByText("无法读取代理设置");
    expect((input as HTMLInputElement).value).toBe("42");
    invoke.mockResolvedValueOnce(settings(true, "confirmed-3"));
    fireEvent.click(screen.getByRole("button", { name: "重新读取设置" }));
    await waitFor(() => {
      expect(input.matches(":disabled")).toBe(false);
      expect((input as HTMLInputElement).value).toBe("600");
    });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(save).toHaveBeenLastCalledWith(expect.objectContaining({ expectedRevision: "confirmed-3" })),
    );
  });

  it("disables rectifier descendants for keyboard users while leaving its master switch available", async () => {
    const data = settings();
    data.rectifierConfig.enabled = false;
    invoke.mockResolvedValue(data);
    render(<ProxyAdvanced />);
    const master = await screen.findByRole("switch", { name: "启用整流器" });
    expect(master.matches(":disabled")).toBe(false);
    expect(screen.getByRole("switch", { name: "Thinking Signature 修复" }).matches(":disabled")).toBe(true);
    fireEvent.click(master);
    expect(screen.getByRole("switch", { name: "Thinking Signature 修复" }).matches(":disabled")).toBe(false);
  });
});
