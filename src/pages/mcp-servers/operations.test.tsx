import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import McpServers from "../McpServers";
import { showToast } from "../../components/Toast";
import { setLocale } from "../../lib/i18n";
import type { CodeEditorProps } from "../../components/CodeEditor";
import type { McpServer } from "./helpers";
import type { DetectedTool } from "../../types/skills";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../../components/DeferredCodeEditor", () => ({
  default: (props: CodeEditorProps) => <textarea aria-label="Preview" value={props.value} readOnly={props.readOnly} />,
}));
const page = vi.hoisted(() => ({
  servers: [
    {
      id: "source-one",
      name: "Fixture",
      command: "node",
      args: "[]",
      env: "{}",
      transport: "stdio",
      status: "active",
      source: "local",
      package_name: null,
      version: null,
      config_path: null,
      origin: { native_name: "Fixture", revision: "rev", bindings: [] },
    },
  ],
  tools: [] as DetectedTool[],
  selected: null as McpServer | null,
  loading: false,
  loadError: null,
  serverAppStatus: {},
  appStatusLoading: false,
  setServers: vi.fn(),
  setSelected: vi.fn(),
  setServerAppStatus: vi.fn(),
  loadPageData: vi.fn(),
}));
vi.mock("./usePageData", () => ({ usePageData: () => page }));
beforeEach(() => {
  setLocale("en");
  vi.mocked(invoke).mockReset();
  vi.mocked(showToast).mockClear();
  page.loadPageData.mockReset().mockResolvedValue(undefined);
});
afterEach(() => {
  cleanup();
  page.selected = null;
  page.tools = [];
  vi.restoreAllMocks();
});

it("forces a fresh catalog read after restoring a library entry", async () => {
  page.selected = { ...page.servers[0], status: "archived", source: "claude" };
  page.tools = [
    {
      id: "claude",
      name: "Claude Code",
      installed: true,
      config_path: "",
      mcp_config_path: "",
      skills_dir: "",
      install_command: "",
      install_url: "",
    },
  ];
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "get_mcp_sync_statuses") return { "source-one": { claude: { state: "missing", disabled: false } } };
    if (command === "sync_mcp_server_to_tool") return;
    throw Error("unexpected command");
  });
  renderPage();
  fireEvent.click(screen.getByRole("tab", { name: "Restore" }));
  const restore = screen.getByRole("button", { name: "Restore to Claude Code" });
  await waitFor(() => expect(restore.matches(":disabled")).toBe(false));
  fireEvent.click(restore);
  await waitFor(() => expect(page.loadPageData).toHaveBeenCalledWith({ force: true }));
});
function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <McpServers />
    </QueryClientProvider>,
  );
}

it("shows a safe environment error and allows a successful retry", async () => {
  vi.mocked(invoke)
    .mockRejectedValueOnce(new Error("private-token"))
    .mockResolvedValueOnce([{ name: "node", display_name: "Node.js", installed: true, version: "24" }]);
  renderPage();
  fireEvent.click(screen.getByRole("button", { name: "Env Check" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Could not read the runtime environment");
  expect(screen.queryByText("private-token")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(await screen.findByText("Node.js")).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
  expect(invoke).toHaveBeenCalledTimes(2);
});

it("reports health failures without exposing native error details and permits retry", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("private-token")).mockResolvedValueOnce([]);
  renderPage();
  const check = screen.getByRole("button", { name: "Health Check" });
  fireEvent.click(check);
  await waitFor(() =>
    expect(showToast).toHaveBeenCalledWith("error", "Health check failed. Server status is unknown. Please retry."),
  );
  await waitFor(() => expect(check.matches(":disabled")).toBe(false));
  fireEvent.click(check);
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
  expect(showToast).toHaveBeenCalledTimes(1);
});

it("keeps an installing wizard open on Escape, prevents duplicate installation and retains failed drafts", async () => {
  let reject!: (error: Error) => void;
  vi.mocked(invoke).mockImplementation(
    () =>
      new Promise((_resolve, no) => {
        reject = no;
      }),
  );
  vi.spyOn(console, "error").mockImplementation(() => {});
  renderPage();
  fireEvent.click(screen.getByRole("button", { name: "Install Wizard" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Server Name" }), { target: { value: "new-service" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Command" }), { target: { value: "node" } });
  fireEvent.click(screen.getByRole("button", { name: "Next" }));
  fireEvent.click(screen.getByRole("button", { name: "Next" }));
  const install = screen.getByRole("button", { name: "Install" });
  fireEvent.click(install);
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
  act(() => window.dispatchEvent(new Event("cchub-shortcut-escape")));
  expect(screen.getByText("MCP Install Wizard")).toBeTruthy();
  fireEvent.click(install);
  expect(invoke).toHaveBeenCalledTimes(1);
  await act(async () => reject(new Error("fixture failure")));
  expect(screen.getByRole("button", { name: "Install" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Back" }));
  fireEvent.click(screen.getByRole("button", { name: "Back" }));
  expect((screen.getByRole("textbox", { name: "Server Name" }) as HTMLInputElement).value).toBe("new-service");
});

it("keeps removal confirmation open after failure and sends a pinned source revision", async () => {
  vi.spyOn(console, "error").mockImplementation(() => {});
  vi.mocked(invoke).mockRejectedValue(new Error("fixture failure"));
  renderPage();
  fireEvent.click(screen.getByRole("button", { name: "Remove" }));
  const dialog = await screen.findByRole("dialog");
  const confirm = dialog.querySelector<HTMLButtonElement>("button:last-child")!;
  fireEvent.click(confirm);
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("uninstall_mcp_server", { name: "source-one", revision: "rev" }),
  );
  await waitFor(() => expect(confirm.disabled).toBe(false));
  expect(screen.getByRole("dialog")).toBeTruthy();
});
