import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { en } from "../../lib/i18n/en";
import type { CodeEditorProps } from "../../components/CodeEditor";
import type { McpServer } from "./helpers";
import McpServerDetailPanel from "./DetailPanel";

vi.mock("../../components/DeferredCodeEditor", () => ({
  default: (props: CodeEditorProps) => (
    <textarea aria-label={props.ariaLabel} readOnly={props.readOnly} value={props.value} />
  ),
}));
afterEach(cleanup);

it("shows remote addresses and headers consistently with the editor", () => {
  render(
    <McpServerDetailPanel
      {...props()}
      selected={{
        ...server,
        transport: "http",
        command: "https://example.test/mcp",
        env: '{"Accept":"application/json"}',
      }}
    />,
  );
  expect(screen.getByText("Server URL")).toBeTruthy();
  expect(screen.queryByText("Command")).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "Configuration" }));
  expect(screen.queryByRole("textbox", { name: "Arguments" })).toBeNull();
  expect((screen.getByRole("textbox", { name: "Headers" }) as HTMLTextAreaElement).value).toContain(
    '"Accept": "application/json"',
  );
});

const server: McpServer = {
  id: "one",
  name: "Fixture",
  command: "node",
  args: '["server.js"]',
  env: "{}",
  status: "active",
  transport: "stdio",
  source: "local",
  package_name: null,
  version: null,
  config_path: "C:/fixture/config.json",
};
const props = () => ({
  selected: server,
  i: en,
  zh: false,
  copied: false,
  copyConfig: vi.fn(),
  startEdit: vi.fn(),
  saveSuccess: false,
  getSourceBadge: () => "badge-muted",
  getSourceLabel: () => "Local",
  installedTools: [
    {
      id: "claude",
      name: "Claude Code",
      config_path: "",
      skills_dir: "",
      mcp_config_path: "",
      installed: true,
      install_command: "",
      install_url: "",
    },
    {
      id: "codex",
      name: "Codex CLI",
      config_path: "",
      skills_dir: "",
      mcp_config_path: "",
      installed: true,
      install_command: "",
      install_url: "",
    },
  ],
  toolSyncStatus: { claude: true, codex: false },
  syncingTo: null as string | null,
  toggleToolSync: vi.fn(),
  onClose: vi.fn(),
});

it("links the panel to one tabbable tab and supports arrow wrap, Home and End", () => {
  render(<McpServerDetailPanel {...props()} />);
  const tabs = screen.getAllByRole("tab");
  const panel = screen.getByRole("tabpanel");
  const selected = (index: number) => {
    expect(tabs.map((tab) => tab.tabIndex)).toEqual(tabs.map((_, i) => (i === index ? 0 : -1)));
    expect(tabs[index].getAttribute("aria-selected")).toBe("true");
    expect(panel.getAttribute("aria-labelledby")).toBe(tabs[index].id);
    expect(tabs[index].getAttribute("aria-controls")).toBe(panel.id);
  };
  selected(0);
  tabs[0].focus();
  fireEvent.keyDown(tabs[0], { key: "ArrowRight" });
  selected(1);
  expect(document.activeElement).toBe(tabs[1]);
  expect((screen.getByRole("textbox", { name: "Arguments" }) as HTMLTextAreaElement).value).toBe('[\n  "server.js"\n]');
  fireEvent.keyDown(tabs[1], { key: "End" });
  selected(2);
  fireEvent.keyDown(tabs[2], { key: "ArrowRight" });
  selected(0);
  fireEvent.keyDown(tabs[0], { key: "ArrowLeft" });
  selected(2);
  fireEvent.keyDown(tabs[2], { key: "Home" });
  selected(0);
});

it("shows unknown status and a retry action without enabling destructive sync actions", () => {
  const initial = props();
  const retry = vi.fn();
  render(<McpServerDetailPanel {...initial} toolSyncStatus={{}} statusError refreshStatus={retry} />);
  fireEvent.click(screen.getByRole("tab", { name: "Sync" }));
  expect(screen.getByRole("alert").textContent).toContain("Sync status is unavailable");
  expect(screen.getAllByText("Unknown status")).toHaveLength(2);
  for (const name of ["Sync to Claude Code", "Sync to Codex CLI"]) {
    const button = screen.getByRole("button", { name }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    fireEvent.click(button);
  }
  expect(initial.toggleToolSync).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(retry).toHaveBeenCalledTimes(1);
});

it("resets the detail to overview when selecting another server and preserves malformed raw JSON", () => {
  const initial = props();
  const view = render(
    <McpServerDetailPanel {...initial} selected={{ ...server, args: "invalid raw args", env: "invalid raw env" }} />,
  );
  fireEvent.click(screen.getByRole("tab", { name: "Configuration" }));
  expect((screen.getByRole("textbox", { name: "Arguments" }) as HTMLTextAreaElement).value).toBe("invalid raw args");
  expect((screen.getByRole("textbox", { name: "Environment" }) as HTMLTextAreaElement).value).toBe("invalid raw env");
  view.rerender(<McpServerDetailPanel {...initial} selected={{ ...server, id: "two", name: "Other" }} />);
  expect(screen.getByRole("tab", { name: "Overview" }).getAttribute("aria-selected")).toBe("true");
  expect(screen.queryByRole("textbox")).toBeNull();
});

it("uses shared actions and serializes tool-sync controls while an operation is pending", () => {
  const initial = props();
  const view = render(<McpServerDetailPanel {...initial} />);
  fireEvent.click(screen.getByRole("button", { name: "Copy configuration" }));
  fireEvent.click(screen.getByRole("button", { name: "Edit Config" }));
  fireEvent.click(screen.getByRole("button", { name: "Close details" }));
  expect(initial.copyConfig).toHaveBeenCalledTimes(1);
  expect(initial.startEdit).toHaveBeenCalledWith(server);
  expect(initial.onClose).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("tab", { name: "Sync" }));
  fireEvent.click(screen.getByRole("button", { name: "Sync to Codex CLI" }));
  expect(initial.toggleToolSync).toHaveBeenCalledWith("codex");
  view.rerender(<McpServerDetailPanel {...initial} syncingTo="codex" />);
  for (const label of ["Remove sync Claude Code", "Sync to Codex CLI"]) {
    const button = screen.getByRole("button", { name: label }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(button.getAttribute("data-slot")).toBe("button");
  }
  expect(screen.getByRole("button", { name: "Sync to Codex CLI" }).getAttribute("aria-busy")).toBe("true");
});
