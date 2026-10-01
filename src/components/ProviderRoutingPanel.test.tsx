import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ProviderRoutingPanel from "./ProviderRoutingPanel";
import { setLocale } from "../lib/i18n";
import type { RoutingDocument } from "../lib/providerRouting";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("./Toast", () => ({ showToast: vi.fn() }));
// Test draft ownership independently of Radix's popup positioning; the actual
// shared selectors are inspected in renderer acceptance.
vi.mock("./ui/simple-select", () => ({
  SimpleSelect: ({
    value,
    options,
    onValueChange,
    ariaLabel,
    disabled,
  }: {
    value: string;
    options: { value: string; label: string; disabled?: boolean }[];
    onValueChange: (value: string) => void;
    ariaLabel: string;
    disabled: boolean;
  }) => (
    <select
      aria-label={ariaLabel}
      value={value}
      disabled={disabled}
      onChange={(event) => onValueChange(event.target.value)}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value} disabled={option.disabled}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

function document(): RoutingDocument {
  return {
    revision: "saved-revision",
    policy: {
      enabled: true,
      defaultGroupId: "g",
      groups: [
        {
          id: "g",
          name: "Saved group",
          mode: "ordered",
          members: [
            { kind: "profile", profileId: "p2" },
            { kind: "profile", profileId: "p1" },
          ],
          pickedProfileId: null,
        },
      ],
      rules: [],
    },
  };
}
const profiles = [
  { providerId: "p1", providerName: "First profile" },
  { providerId: "p2", providerName: "Second profile" },
];
beforeEach(() => {
  setLocale("zh");
  vi.clearAllMocks();
  invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
    if (command === "get_provider_routing") return document();
    if (command === "get_available_providers_for_failover") return profiles;
    if (command === "set_provider_routing") return { revision: "new-revision", policy: args?.policy };
    if (command === "preview_provider_routing")
      return { groupId: "g", ruleId: null, profileIds: ["p2", "p1"], reason: "defaultGroup" };
    throw new Error(`Unexpected command: ${command}`);
  });
});
afterEach(cleanup);

describe("provider routing editor", () => {
  it("saves exact revisions, protects unsaved drafts on tool changes and supports undo", async () => {
    render(<ProviderRoutingPanel />);
    const name = await screen.findByRole("textbox", { name: "分组名称" });
    fireEvent.change(name, { target: { value: "Draft group" } });
    expect(screen.getByRole("combobox", { name: "路由工具" }).matches(":disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "保存路由" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        "set_provider_routing",
        expect.objectContaining({
          appType: "claude",
          expectedRevision: "saved-revision",
          policy: expect.objectContaining({
            groups: expect.arrayContaining([expect.objectContaining({ name: "Draft group" })]),
          }),
        }),
      ),
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "保存路由" }).matches(":disabled")).toBe(true));
    fireEvent.change(name, { target: { value: "Another draft" } });
    fireEvent.click(screen.getByRole("button", { name: "撤销修改" }));
    expect((name as HTMLInputElement).value).toBe("Draft group");
  });

  it("retains a conflicting draft and never retries an overwrite", async () => {
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation((command: string, args: Record<string, unknown>) =>
      command === "set_provider_routing"
        ? Promise.reject(new Error("Routing settings changed elsewhere"))
        : original(command, args),
    );
    render(<ProviderRoutingPanel />);
    const name = await screen.findByRole("textbox", { name: "分组名称" });
    fireEvent.change(name, { target: { value: "Retained draft" } });
    fireEvent.click(screen.getByRole("button", { name: "保存路由" }));
    await screen.findByRole("alert");
    expect((name as HTMLInputElement).value).toBe("Retained draft");
    expect(invoke.mock.calls.filter(([command]) => command === "set_provider_routing")).toHaveLength(1);
  });

  it("previews draft rules without saving or making an external model call", async () => {
    render(<ProviderRoutingPanel />);
    await screen.findByRole("textbox", { name: "分组名称" });
    fireEvent.click(screen.getByText("预览规则与初始顺序"));
    fireEvent.change(screen.getByRole("textbox", { name: "预览请求模型" }), { target: { value: "fixture-model" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "预览请求大小" }), { target: { value: "4096" } });
    fireEvent.click(screen.getByRole("button", { name: "预览路由" }));
    await screen.findByText("使用默认分组");
    expect(invoke).toHaveBeenCalledWith(
      "preview_provider_routing",
      expect.objectContaining({ request: expect.objectContaining({ model: "fixture-model" }), requestBytes: 4096 }),
    );
    expect(invoke.mock.calls.some(([command]) => command === "set_provider_routing")).toBe(false);
  });

  it("orders members and edits manual selection using shared form state", async () => {
    render(<ProviderRoutingPanel />);
    await screen.findByRole("textbox", { name: "分组名称" });
    fireEvent.click(screen.getByRole("button", { name: "上移成员 2" }));
    fireEvent.change(screen.getByRole("combobox", { name: "分组选择策略" }), { target: { value: "manual" } });
    expect((screen.getByRole("combobox", { name: "固定配置" }) as HTMLSelectElement).value).toBe("p1");
    fireEvent.click(screen.getByRole("button", { name: "保存路由" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        "set_provider_routing",
        expect.objectContaining({
          policy: expect.objectContaining({
            groups: expect.arrayContaining([
              expect.objectContaining({
                mode: "manual",
                pickedProfileId: "p1",
                members: [
                  { kind: "profile", profileId: "p1" },
                  { kind: "profile", profileId: "p2" },
                ],
              }),
            ]),
          }),
        }),
      ),
    );
  });

  it("ignores an obsolete preview after the draft changes", async () => {
    let resolvePreview!: (value: unknown) => void;
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation((command: string, args: Record<string, unknown>) =>
      command === "preview_provider_routing"
        ? new Promise((resolve) => {
            resolvePreview = resolve;
          })
        : original(command, args),
    );
    render(<ProviderRoutingPanel />);
    const name = await screen.findByRole("textbox", { name: "分组名称" });
    fireEvent.click(screen.getByText("预览规则与初始顺序"));
    fireEvent.click(screen.getByRole("button", { name: "预览路由" }));
    fireEvent.change(name, { target: { value: "Changed draft" } });
    resolvePreview({ groupId: "g", ruleId: null, profileIds: ["p2"], reason: "defaultGroup" });
    await waitFor(() => expect(screen.queryByText("使用默认分组")).toBeNull());
  });

  it("exposes load failures with retry instead of substituting default settings", async () => {
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation((command: string, args: Record<string, unknown>) =>
      command === "get_provider_routing" ? Promise.reject(new Error("Cannot read routing")) : original(command, args),
    );
    render(<ProviderRoutingPanel />);
    await screen.findByRole("alert");
    expect(screen.queryByRole("button", { name: "保存路由" })).toBeNull();
    invoke.mockImplementation(original);
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await screen.findByRole("textbox", { name: "分组名称" });
  });
});
