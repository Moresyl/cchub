import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ModelsDevSyncPanel from "./ModelsDevSyncPanel";
import { rememberDraft } from "./models-dev-sync/draft";
import type { CatalogEntry, SyncConfig } from "./models-dev-sync/types";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("../lib/i18n", () => ({ getLocale: () => "zh" }));
const initial: SyncConfig = {
  autoSyncEnabled: false,
  includeCommonModels: true,
  selectedModelKeys: [],
  excludedCommonModelKeys: [],
  lastSyncAt: null,
  lastSyncError: null,
};
const catalog: CatalogEntry[] = Array.from({ length: 205 }, (_, index) => ({
  key: `demo/model-${index + 1}`,
  modelId: `model-${index + 1}`,
  modelName: `模型 ${index + 1}`,
  providerId: "demo",
  providerName: "团队",
  releaseDate: "",
  isCommon: index < 6,
  input: 1,
  output: 2,
  cacheRead: 0.5,
  cacheWrite: 0.25,
}));
let config: SyncConfig;
beforeEach(() => {
  rememberDraft(null);
  config = structuredClone(initial);
  invoke.mockReset().mockImplementation(async (command, args) => {
    if (command === "get_models_dev_sync_config")
      return { config: structuredClone(config), configPath: "C:/demo/pricing.json" };
    if (command === "get_models_dev_catalog") return catalog;
    if (command === "save_models_dev_sync_config") {
      config = args.config;
      return { config: structuredClone(config), configPath: "C:/demo/pricing.json" };
    }
    if (command === "sync_models_dev_pricing")
      return { skipped: false, selected: 6, imported: 6, changed: 6, syncedAt: 100 };
    throw new Error("Unexpected command");
  });
});
afterEach(() => {
  cleanup();
  rememberDraft(null);
});
async function picker() {
  render(<ModelsDevSyncPanel />);
  fireEvent.click(await screen.findByRole("button", { name: "选择模型" }));
  await screen.findByRole("checkbox", { name: "模型 1" });
}
describe("model pricing workspace", () => {
  it("reviews an external conflict without writing and saves independent changes together", async () => {
    await picker();
    fireEvent.click(screen.getByRole("checkbox", { name: "模型 7" }));
    config = { ...config, includeCommonModels: false, selectedModelKeys: ["demo/model-8"] };
    const saved = structuredClone(config);
    invoke.mockRejectedValueOnce("PRICING_SETTINGS_CONFLICT");
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await screen.findByRole("button", { name: "核对更改" });
    fireEvent.click(screen.getByRole("button", { name: "核对更改" }));
    const dialog = await screen.findByRole("dialog", { name: "核对价格设置更改" });
    await within(dialog).findByText("demo/model-8");
    expect(invoke.mock.calls.filter(([command]) => command === "save_models_dev_sync_config")).toHaveLength(1);
    fireEvent.click(within(dialog).getByRole("button", { name: "应用核对结果" }));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("heading", { name: "模型价格同步" })));
    expect(invoke.mock.calls.filter(([command]) => command === "save_models_dev_sync_config")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await waitFor(() => expect(config.selectedModelKeys).toEqual(["demo/model-7", "demo/model-8"]));
    expect(config.includeCommonModels).toBe(false);
    expect(invoke).toHaveBeenLastCalledWith("save_models_dev_sync_config", {
      config: { ...saved, selectedModelKeys: ["demo/model-7", "demo/model-8"] },
      expectedConfig: saved,
    });
  });
  it("renders only the six backend common selections and browses every catalog entry", async () => {
    await picker();
    const list = document.querySelector(".pricing-model-list") as HTMLElement;
    expect(within(list).getAllByRole("checkbox")).toHaveLength(24);
    expect(
      within(list)
        .getAllByRole("checkbox")
        .filter((item) => item.getAttribute("aria-checked") === "true"),
    ).toHaveLength(6);
    expect(screen.getByRole("checkbox", { name: "模型 7" }).getAttribute("aria-checked")).toBe("false");
    for (let page = 0; page < 8; page++) fireEvent.click(screen.getByRole("button", { name: "下一页" }));
    expect(screen.getByRole("checkbox", { name: "模型 205" })).toBeDefined();
    expect((screen.getByRole("button", { name: "下一页" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "搜索模型或供应商" }), { target: { value: "model-181" } });
    expect(screen.getByRole("checkbox", { name: "模型 181" })).toBeDefined();
    expect(within(list).getAllByRole("checkbox")).toHaveLength(1);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "absent" } });
    expect(screen.getByText("没有匹配模型")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "清除筛选" }));
    expect(screen.getByRole("checkbox", { name: "模型 1" })).toBeDefined();
  });

  it("retains model selections on status refresh and saves flags and selections together using the shortcut", async () => {
    await picker();
    fireEvent.click(screen.getByRole("checkbox", { name: "模型 7" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "启动时自动同步" }));
    fireEvent.click(screen.getByRole("button", { name: "刷新同步状态" }));
    await waitFor(() =>
      expect((screen.getByRole("button", { name: "保存设置" }) as HTMLButtonElement).disabled).toBe(false),
    );
    expect(screen.getByRole("checkbox", { name: "模型 7" }).getAttribute("aria-checked")).toBe("true");
    expect(invoke.mock.calls.some(([command]) => command === "save_models_dev_sync_config")).toBe(false);
    await act(async () => {
      window.dispatchEvent(new CustomEvent("cchub-shortcut-save"));
    });
    expect(config.autoSyncEnabled).toBe(true);
    expect(config.selectedModelKeys).toEqual(["demo/model-7"]);
    expect(invoke.mock.calls.some(([command]) => command === "sync_models_dev_pricing")).toBe(false);
  });

  it("shows an accessible retry after catalog failure without leaking internal errors", async () => {
    invoke
      .mockImplementationOnce(async () => ({ config: initial, configPath: "C:/demo/pricing.json" }))
      .mockRejectedValueOnce("token=private-audit; C:/private/config");
    render(<ModelsDevSyncPanel />);
    fireEvent.click(await screen.findByRole("button", { name: "选择模型" }));
    expect(await screen.findByRole("alert")).toBeDefined();
    expect(document.body.textContent).not.toContain("token=private-audit");
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    expect(await screen.findByRole("checkbox", { name: "模型 1" })).toBeDefined();
  });

  it("uses a shared confirmation before discarding a draft and preserves it when reloading fails", async () => {
    await picker();
    fireEvent.click(screen.getByRole("checkbox", { name: "模型 7" }));
    fireEvent.click(screen.getByRole("button", { name: "放弃更改" }));
    expect(screen.getByRole("dialog")).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "继续编辑" }));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "放弃更改" })));
    expect(screen.getByRole("checkbox", { name: "模型 7" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "放弃更改" }));
    invoke.mockRejectedValueOnce("private error");
    fireEvent.click(screen.getByRole("button", { name: "放弃并重新加载" }));
    await screen.findByText("同步设置读取失败");
    expect(screen.getByRole("checkbox", { name: "模型 7" }).getAttribute("aria-checked")).toBe("true");
    expect(document.body.textContent).not.toContain("private error");
  });
});
