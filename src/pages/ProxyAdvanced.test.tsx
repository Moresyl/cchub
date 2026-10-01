import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
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
  };
}

beforeEach(() => {
  setLocale("zh");
  vi.clearAllMocks();
  invoke.mockImplementation(async (command: string) =>
    command === "get_optimizer_config" ? config() : { enabled: true, thinkingSignature: true, thinkingBudget: true },
  );
  save.mockResolvedValue(undefined);
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
    expect((input as HTMLInputElement).value).toBe("600");
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
        }),
      ),
    );
    expect(input.getAttribute("data-control-size")).toBe("md");
  });

  it("disables optimizer descendants for keyboard and pointer users when the master switch is off", async () => {
    invoke.mockImplementation(async (command: string) =>
      command === "get_optimizer_config"
        ? config(false)
        : { enabled: true, thinkingSignature: true, thinkingBudget: true },
    );
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
});
