import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import McpServers from "../McpServers";
import { setLocale } from "../../lib/i18n";
import type { CodeEditorProps } from "../../components/CodeEditor";

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
  tools: [],
  selected: null,
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
  page.loadPageData.mockResolvedValue(undefined);
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <McpServers />
    </QueryClientProvider>,
  );
}

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
