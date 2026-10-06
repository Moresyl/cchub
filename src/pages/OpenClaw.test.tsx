import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import OpenClaw from "./OpenClaw";
import { setLocale, t } from "../lib/i18n";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../components/CodeEditor", () => ({
  default: ({ value, ariaLabel }: { value: string; ariaLabel: string }) => <pre aria-label={ariaLabel}>{value}</pre>,
}));
const responses: Record<string, unknown> = {
  get_openclaw_status: { installed: true, configPath: "C:/fixture/openclaw.json" },
  scan_openclaw_health: [],
  get_openclaw_env: {
    API_KEY: "placeholder",
    shellEnv: { enabled: true, timeoutMs: 100 },
    vars: { KEEP: "value" },
    count: 3,
  },
  get_openclaw_tools: { profile: "custom-profile", allow: ["read"], deny: ["exec"], loopDetection: { enabled: true } },
  get_openclaw_agents_defaults: {
    model: { primary: "provider/main", fallbacks: ["provider/backup"], extension: true },
    models: { "provider/main": { alias: "Main", custom: { enabled: true } } },
    workspace: "/fixture",
    compaction: { mode: "safeguard" },
  },
};
const invokeMock = vi.mocked(invoke);
const failures = new Set<string>();
function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <OpenClaw />
    </QueryClientProvider>,
  );
}
function panel() {
  return screen.getByRole("tabpanel");
}
function save() {
  return within(panel()).getByRole("button", { name: t().openClaw.save });
}
async function select(name: string) {
  fireEvent.click(await screen.findByRole("tab", { name }));
}
function calls(command: string) {
  return invokeMock.mock.calls.filter(([name]) => name === command);
}
beforeEach(() => {
  setLocale("zh");
  failures.clear();
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (command) => {
    if (failures.has(command)) throw new Error("PRIVATE_DETAIL must not leak");
    return structuredClone(responses[command]);
  });
});
afterEach(cleanup);

describe("OpenClaw configuration workspace", () => {
  it("keeps the loading state until the initial status request finishes", async () => {
    let resolve!: (value: unknown) => void;
    const pending = new Promise((done) => {
      resolve = done;
    });
    invokeMock.mockImplementation(async (command) =>
      command === "get_openclaw_status" ? pending : structuredClone(responses[command]),
    );
    renderPage();
    expect(screen.getByText(t().openClaw.loading)).toBeTruthy();
    expect(screen.queryByText(t().openClaw.notInstalled)).toBeNull();
    await act(async () => {
      resolve(responses.get_openclaw_status);
      await pending;
    });
    await screen.findByRole("textbox", { name: `${t().openClaw.variableName} 1` });
  });
  it("can remove the final string variable without changing structured settings", async () => {
    renderPage();
    const i = t().openClaw;
    fireEvent.click(await screen.findByRole("button", { name: `${i.removeVar} 1` }));
    expect(screen.getByText(i.emptyEnv)).toBeTruthy();
    fireEvent.click(save());
    await waitFor(() => expect(calls("set_openclaw_env")).toHaveLength(1));
    expect(calls("set_openclaw_env")[0][1]).toEqual({
      env: { shellEnv: { enabled: true, timeoutMs: 100 }, vars: { KEEP: "value" }, count: 3 },
    });
  });
  it("handles omitted backend arrays and optional model settings", async () => {
    invokeMock.mockImplementation(async (command) =>
      command === "get_openclaw_tools"
        ? { extension: true }
        : command === "get_openclaw_agents_defaults"
          ? { workspace: "/keep" }
          : structuredClone(responses[command]),
    );
    const i = t().openClaw;
    renderPage();
    await select(i.toolsTab);
    await within(panel()).findByRole("textbox", { name: `${i.allowList}: ${i.toolName}` });
    expect(within(panel()).getAllByText(i.emptyList)).toHaveLength(2);
    await select(i.agentsTab);
    fireEvent.change(await screen.findByRole("textbox", { name: i.primaryModel }), { target: { value: "local/new" } });
    fireEvent.click(save());
    await waitFor(() => expect(calls("set_openclaw_agents_defaults")).toHaveLength(1));
    expect(calls("set_openclaw_agents_defaults")[0][1]).toEqual({
      defaults: { workspace: "/keep", model: { primary: "local/new", fallbacks: [] }, models: null },
    });
  });
  it("does not add a tool while the input method is composing", async () => {
    const i = t().openClaw;
    renderPage();
    await select(i.toolsTab);
    const input = await screen.findByRole("textbox", { name: `${i.allowList}: ${i.toolName}` });
    fireEvent.change(input, { target: { value: "composing" } });
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    expect(within(panel()).queryByText("composing")).toBeNull();
    expect(calls("set_openclaw_tools")).toHaveLength(0);
  });
  it("separates unreadable status from an absent installation and retries", async () => {
    failures.add("get_openclaw_status");
    renderPage();
    await screen.findByRole("alert");
    expect(screen.queryByText(t().openClaw.notInstalled)).toBeNull();
    expect(screen.queryByText(/PRIVATE_DETAIL/)).toBeNull();
    failures.clear();
    fireEvent.click(screen.getByRole("button", { name: t().openClaw.retry }));
    await screen.findByRole("tab", { name: t().openClaw.envTab });
  });
  it("shows absent configuration only after a successful status check", async () => {
    invokeMock.mockImplementation(async (command) =>
      command === "get_openclaw_status" ? { installed: false, configPath: "/missing" } : [],
    );
    renderPage();
    await screen.findByText(t().openClaw.notInstalled);
    expect(screen.queryByRole("tab")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: t().openClaw.checkAgain }));
    await waitFor(() => expect(calls("get_openclaw_status")).toHaveLength(2));
  });
  it("keeps configuration usable when only health scanning fails", async () => {
    failures.add("scan_openclaw_health");
    renderPage();
    const field = await screen.findByRole("textbox", { name: `${t().openClaw.variableValue} 1` });
    expect(screen.getByText(t().openClaw.healthFailed)).toBeTruthy();
    expect(screen.queryByText(t().openClaw.notInstalled)).toBeNull();
    fireEvent.change(field, { target: { value: "new-value" } });
    fireEvent.click(save());
    await waitFor(() => expect(calls("set_openclaw_env")).toHaveLength(1));
    failures.clear();
    fireEvent.click(screen.getByRole("button", { name: t().openClaw.retryHealth }));
    await waitFor(() => expect(screen.queryByText(t().openClaw.healthFailed)).toBeNull());
  });
  it.each([
    ["envTab", "get_openclaw_env"],
    ["toolsTab", "get_openclaw_tools"],
    ["agentsTab", "get_openclaw_agents_defaults"],
  ] as const)("blocks saving defaults after %s fails to load", async (tab, command) => {
    failures.add(command);
    renderPage();
    await select(t().openClaw[tab]);
    await within(panel()).findByRole("alert");
    expect(within(panel()).queryByRole("button", { name: t().openClaw.save })).toBeNull();
    expect(calls(command.replace("get_", "set_"))).toHaveLength(0);
    failures.clear();
    fireEvent.click(within(panel()).getByRole("button", { name: t().openClaw.retry }));
    await within(panel()).findByRole("button", { name: t().openClaw.save });
    expect(calls(command)).toHaveLength(2);
  });
  it("preserves structured env settings and original types when editing a string", async () => {
    renderPage();
    fireEvent.change(await screen.findByRole("textbox", { name: `${t().openClaw.variableValue} 1` }), {
      target: { value: "changed" },
    });
    fireEvent.click(save());
    await waitFor(() => expect(calls("set_openclaw_env")).toHaveLength(1));
    expect(calls("set_openclaw_env")[0][1]).toEqual({
      env: { ...(responses.get_openclaw_env as object), API_KEY: "changed" },
    });
    await waitFor(() => expect((save() as HTMLButtonElement).disabled).toBe(true));
  });
  it("retains drafts across tabs and health/status refreshes", async () => {
    renderPage();
    const i = t().openClaw;
    fireEvent.change(await screen.findByRole("textbox", { name: `${i.variableValue} 1` }), {
      target: { value: "draft" },
    });
    await select(i.toolsTab);
    await select(i.agentsTab);
    await select(i.envTab);
    expect((screen.getByRole("textbox", { name: `${i.variableValue} 1` }) as HTMLInputElement).value).toBe("draft");
    fireEvent.click(screen.getByRole("button", { name: i.checkAgain }));
    await waitFor(() => expect(calls("get_openclaw_status")).toHaveLength(2));
    expect(calls("get_openclaw_env")).toHaveLength(1);
    expect((screen.getByRole("textbox", { name: `${i.variableValue} 1` }) as HTMLInputElement).value).toBe("draft");
  });
  it.each([false, true])("retains drafts when a later status check fails or reports missing=%s", async (missing) => {
    renderPage();
    const i = t().openClaw;
    fireEvent.change(await screen.findByRole("textbox", { name: `${i.variableValue} 1` }), {
      target: { value: "draft" },
    });
    if (missing)
      invokeMock.mockImplementation(async (command) =>
        command === "get_openclaw_status"
          ? { installed: false, configPath: "/missing" }
          : structuredClone(responses[command]),
      );
    else failures.add("get_openclaw_status");
    fireEvent.click(screen.getByRole("button", { name: i.checkAgain }));
    await screen.findByText(missing ? i.configMissing : i.readFailed);
    expect((screen.getByRole("textbox", { name: `${i.variableValue} 1` }) as HTMLInputElement).value).toBe("draft");
    expect((save().closest("fieldset") as HTMLFieldSetElement).disabled).toBe(true);
    fireEvent.submit(save().closest("form")!);
    expect(calls("set_openclaw_env")).toHaveLength(0);
  });
  it.each([
    ["envTab", "set_openclaw_env", "variableValue", "changed"],
    ["toolsTab", "set_openclaw_tools", "toolName", "new-tool"],
    ["agentsTab", "set_openclaw_agents_defaults", "primaryModel", "new-model"],
  ] as const)("retains %s after failed saving and retries", async (tab, command, field, value) => {
    const i = t().openClaw;
    failures.add(command);
    renderPage();
    await select(i[tab]);
    const input =
      field === "variableValue"
        ? await screen.findByRole("textbox", { name: `${i.variableValue} 1` })
        : field === "toolName"
          ? await screen.findByRole("textbox", { name: `${i.allowList}: ${i.toolName}` })
          : await screen.findByRole("textbox", { name: i.primaryModel });
    fireEvent.change(input, { target: { value } });
    if (field === "toolName") fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.click(save());
    await within(panel()).findByText(i.saveFailedDesc);
    expect(screen.queryByText(/PRIVATE_DETAIL/)).toBeNull();
    failures.delete(command);
    fireEvent.click(save());
    await waitFor(() => expect(calls(command)).toHaveLength(2));
    await waitFor(() => expect(within(panel()).queryByText(i.saveFailedDesc)).toBeNull());
  });
  it.each(["", "API_KEY", "shellEnv"])("rejects blank, duplicate or structured setting names: %s", async (key) => {
    const i = t().openClaw;
    renderPage();
    await screen.findByRole("textbox", { name: `${i.variableName} 1` });
    fireEvent.click(screen.getByRole("button", { name: i.addVar }));
    fireEvent.change(screen.getByRole("textbox", { name: `${i.variableName} 2` }), { target: { value: key } });
    expect((save() as HTMLButtonElement).disabled).toBe(true);
    fireEvent.submit(save().closest("form")!);
    expect(calls("set_openclaw_env")).toHaveLength(0);
  });
  it("undoes drafts without writing or rereading configuration", async () => {
    const i = t().openClaw;
    renderPage();
    const field = await screen.findByRole("textbox", { name: `${i.variableValue} 1` });
    fireEvent.change(field, { target: { value: "draft" } });
    fireEvent.click(screen.getByRole("button", { name: i.reset }));
    expect((field as HTMLInputElement).value).toBe("placeholder");
    expect(calls("set_openclaw_env")).toHaveLength(0);
    expect(calls("get_openclaw_env")).toHaveLength(1);
  });
  it("adds tools with Enter without saving, prevents duplicates and preserves extra settings", async () => {
    const i = t().openClaw;
    renderPage();
    await select(i.toolsTab);
    const field = await screen.findByRole("textbox", { name: `${i.allowList}: ${i.toolName}` });
    fireEvent.change(field, { target: { value: "read" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(within(panel()).getAllByText("read")).toHaveLength(1);
    fireEvent.change(field, { target: { value: "new" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(calls("set_openclaw_tools")).toHaveLength(0);
    fireEvent.click(save());
    await waitFor(() => expect(calls("set_openclaw_tools")).toHaveLength(1));
    expect(calls("set_openclaw_tools")[0][1]).toEqual({
      tools: { ...(responses.get_openclaw_tools as object), allow: ["read", "new"] },
    });
  });
  it("edits primary model and aliases while retaining model and agent extensions", async () => {
    const i = t().openClaw;
    renderPage();
    await select(i.agentsTab);
    fireEvent.change(await screen.findByRole("textbox", { name: i.primaryModel }), {
      target: { value: "provider/new" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: `${i.modelAliases}: provider/main` }), {
      target: { value: "New alias" },
    });
    fireEvent.click(save());
    await waitFor(() => expect(calls("set_openclaw_agents_defaults")).toHaveLength(1));
    expect(calls("set_openclaw_agents_defaults")[0][1]).toEqual({
      defaults: {
        ...(responses.get_openclaw_agents_defaults as object),
        model: { primary: "provider/new", fallbacks: ["provider/backup"], extension: true },
        models: { "provider/main": { alias: "New alias", custom: { enabled: true } } },
      },
    });
  });
  it("navigates tabs using arrows, Home and End with correct focus and panel association", async () => {
    const i = t().openClaw;
    renderPage();
    const env = await screen.findByRole("tab", { name: i.envTab });
    env.focus();
    fireEvent.keyDown(env, { key: "ArrowRight" });
    const tools = screen.getByRole("tab", { name: i.toolsTab });
    expect(document.activeElement).toBe(tools);
    expect(tools.getAttribute("aria-selected")).toBe("true");
    expect(panel().getAttribute("aria-labelledby")).toBe(tools.id);
    fireEvent.keyDown(tools, { key: "End" });
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: i.agentsTab }));
    fireEvent.keyDown(document.activeElement!, { key: "Home" });
    expect(document.activeElement).toBe(env);
  });
  it("blocks duplicate writes and edits during a pending save", async () => {
    const i = t().openClaw;
    renderPage();
    const field = await screen.findByRole("textbox", { name: `${i.variableValue} 1` });
    let resolve!: () => void;
    const pending = new Promise<void>((done) => {
      resolve = done;
    });
    invokeMock.mockImplementation(async (command) =>
      command === "set_openclaw_env" ? pending : structuredClone(responses[command]),
    );
    fireEvent.change(field, { target: { value: "pending" } });
    const form = save().closest("form")!;
    fireEvent.submit(form);
    fireEvent.submit(form);
    await waitFor(() => expect(calls("set_openclaw_env")).toHaveLength(1));
    fireEvent.change(field, { target: { value: "ignored" } });
    expect((field as HTMLInputElement).value).toBe("pending");
    await act(async () => {
      resolve();
      await pending;
    });
    expect((field as HTMLInputElement).value).toBe("pending");
  });
});
