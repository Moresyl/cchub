import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { performFetchModels, useModelDiscovery, type ModelDiscoveryContext } from "./modelDiscovery";
import type { ModelInfo } from "../../components/ModelSelector";

const invoke = vi.fn();
const toast = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("../../components/Toast", () => ({ showToast: (...args: unknown[]) => toast(...args) }));

function context(overrides: Partial<ModelDiscoveryContext> = {}): ModelDiscoveryContext {
  return {
    fetchingModels: false,
    draftTool: "claude",
    draftProviderType: "",
    draftOAuthAccountId: "",
    draftApiKey: "fixture-key",
    draftUseFullUrl: false,
    draftBaseUrl: "https://example.test/v1",
    draftCustomUserAgent: "",
    draftRequestHeaders: {},
    localeText: (zh) => zh,
    setFetchingModels: vi.fn(),
    setModelFetchError: vi.fn(),
    setFetchedModelDetails: vi.fn(),
    setFetchedModels: vi.fn(),
    onCatalog: vi.fn(),
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("model discovery", () => {
  it("deduplicates and enriches IDs without discarding valid metadata", async () => {
    invoke.mockResolvedValue([
      { id: " a ", displayName: "Alpha", contextWindow: 128000 },
      { id: "a", displayName: null, outputPrice: "0.02" },
      { id: " " },
      { id: "b" },
    ]);
    const ctx = context();
    await performFetchModels(ctx);
    expect(ctx.setFetchedModels).toHaveBeenCalledWith(["a", "b"]);
    expect(ctx.setFetchedModelDetails).toHaveBeenCalledWith([
      { id: "a", displayName: "Alpha", contextWindow: 128000, outputPrice: "0.02" },
      { id: "b" },
    ]);
    expect(ctx.onCatalog).toHaveBeenCalledWith({
      toolId: "claude",
      models: [{ id: "a", displayName: "Alpha", contextWindow: 128000, outputPrice: "0.02" }, { id: "b" }],
    });
  });

  it.each(["github_copilot", "codex_oauth", "xai_oauth"])("retains display names for %s", async (type) => {
    invoke.mockResolvedValue([{ id: "model", name: "Model name", displayName: "Model name" }]);
    const ctx = context({ draftProviderType: type, draftApiKey: "", draftOAuthAccountId: " account " });
    await performFetchModels(ctx);
    expect(invoke).toHaveBeenCalledWith(
      type === "github_copilot"
        ? "copilot_get_models"
        : type === "codex_oauth"
          ? "get_codex_oauth_models"
          : "get_xai_oauth_models",
      { accountId: "account" },
    );
    expect((vi.mocked(ctx.setFetchedModelDetails).mock.calls[0][0] as ModelInfo[])[0].displayName).toBe("Model name");
  });

  it("keeps a usable catalog when refresh fails and reports the error", async () => {
    invoke.mockRejectedValue(new Error("network unavailable"));
    const ctx = context();
    await performFetchModels(ctx);
    expect(ctx.setFetchedModels).not.toHaveBeenCalled();
    expect(ctx.setFetchedModelDetails).not.toHaveBeenCalled();
    expect(ctx.onCatalog).not.toHaveBeenCalled();
    expect(ctx.setModelFetchError).toHaveBeenCalled();
    expect(ctx.setFetchingModels).toHaveBeenLastCalledWith(false);
  });

  it.each([
    ["claude", { draftApiFormat: "openai_responses" }, "openai_responses"],
    ["openclaw", { draftApiProtocol: "anthropic-messages" }, "anthropic-messages"],
    ["opencode", { draftNpm: "@ai-sdk/google" }, "@ai-sdk/google"],
    ["hermes", { draftHermesProvider: "anthropic" }, "anthropic"],
  ])("uses the selected protocol for %s model discovery", async (tool, fields, protocol) => {
    invoke.mockResolvedValue([
      { id: "a", inputModalities: ["text", "image"], supportedReasoningLevels: ["low", "high"] },
    ]);
    const ctx = context({ draftTool: tool, ...fields });
    await performFetchModels(ctx);
    expect(invoke).toHaveBeenCalledWith(
      "fetch_provider_models_detailed",
      expect.objectContaining({ apiFormat: protocol }),
    );
    expect(ctx.setFetchedModelDetails).toHaveBeenCalledWith([
      { id: "a", inputModalities: ["text", "image"], supportedReasoningLevels: ["low", "high"] },
    ]);
  });

  it("invalidates in-flight catalogs when the selected protocol changes", async () => {
    let resolve!: (models: ModelInfo[]) => void;
    invoke.mockReturnValue(
      new Promise<ModelInfo[]>((done) => {
        resolve = done;
      }),
    );
    const ctx = context({ draftApiFormat: "anthropic" });
    const hook = renderHook(({ protocol }) => useModelDiscovery({ ...ctx, draftApiFormat: protocol }, "provider"), {
      initialProps: { protocol: "anthropic" },
    });
    let pending!: Promise<void>;
    act(() => {
      pending = hook.result.current();
    });
    hook.rerender({ protocol: "openai_chat" });
    vi.mocked(ctx.setFetchedModelDetails).mockClear();
    await act(async () => {
      resolve([{ id: "stale" }]);
      await pending;
    });
    expect(ctx.setFetchedModelDetails).not.toHaveBeenCalled();
  });

  it("does not invoke the backend when credentials or the endpoint are missing", async () => {
    await performFetchModels(context({ draftApiKey: "" }));
    await performFetchModels(context({ draftBaseUrl: "", draftUseFullUrl: true }));
    expect(invoke).not.toHaveBeenCalled();
    expect(toast).toHaveBeenCalledTimes(2);
  });

  it("ignores a late result after changing the editor and blocks simultaneous fetches", async () => {
    let resolve!: (models: ModelInfo[]) => void;
    invoke.mockReturnValue(
      new Promise<ModelInfo[]>((done) => {
        resolve = done;
      }),
    );
    const ctx = context();
    const hook = renderHook(({ scope }) => useModelDiscovery(ctx, scope), { initialProps: { scope: "provider-a" } });
    let pending!: Promise<void>;
    act(() => {
      pending = hook.result.current();
    });
    await act(async () => {
      await hook.result.current();
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    hook.rerender({ scope: "provider-b" });
    vi.mocked(ctx.setFetchedModelDetails).mockClear();
    toast.mockClear();
    await act(async () => {
      resolve([{ id: "stale-model" }]);
      await pending;
    });
    expect(ctx.setFetchedModelDetails).not.toHaveBeenCalled();
    expect(toast).not.toHaveBeenCalled();
    expect(ctx.onCatalog).not.toHaveBeenCalled();
  });

  it("restores only the current tool's saved catalog", () => {
    const ctx = context({ draftModelCatalog: { toolId: "claude", models: [{ id: "saved", contextWindow: 200000 }] } });
    const hook = renderHook(({ tool }) => useModelDiscovery({ ...ctx, draftTool: tool }, "same-profile"), {
      initialProps: { tool: "claude" },
    });
    expect(ctx.setFetchedModels).toHaveBeenLastCalledWith(["saved"]);
    hook.rerender({ tool: "opencode" });
    expect(ctx.setFetchedModels).toHaveBeenLastCalledWith([]);
    expect(ctx.setFetchedModelDetails).toHaveBeenLastCalledWith([]);
  });

  it("ignores a late error after unmounting", async () => {
    let reject!: (error: Error) => void;
    invoke.mockReturnValue(
      new Promise<ModelInfo[]>((_, fail) => {
        reject = fail;
      }),
    );
    const ctx = context();
    const hook = renderHook(() => useModelDiscovery(ctx, "provider"));
    let pending!: Promise<void>;
    act(() => {
      pending = hook.result.current();
    });
    hook.unmount();
    vi.mocked(ctx.setModelFetchError).mockClear();
    await act(async () => {
      reject(new Error("late failure"));
      await pending;
    });
    expect(ctx.setModelFetchError).not.toHaveBeenCalled();
    expect(toast).not.toHaveBeenCalled();
  });
});
