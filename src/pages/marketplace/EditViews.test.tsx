import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { McpEditView } from "./EditViews";
import { setLocale } from "../../lib/i18n";
import type { CodeEditorProps } from "../../components/CodeEditor";

vi.mock("../../components/DeferredCodeEditor", () => ({
  default: (props: CodeEditorProps) => (
    <textarea
      aria-label={props.ariaLabel}
      value={props.value}
      readOnly={props.readOnly}
      onChange={(event) => props.onChange?.(event.target.value)}
    />
  ),
}));
afterEach(cleanup);
const props = () => ({
  locale: "en",
  editingMcp: {
    id: "one",
    name: "Remote",
    command: "https://fixture.test/mcp",
    args: "[]",
    env: "{}",
    transport: "http",
    status: "active",
    source: "local",
    package_name: null,
    version: null,
    config_path: null,
  },
  editCommand: "https://fixture.test/new",
  editArgs: "[]",
  editEnv: "{}",
  originalMcpCommand: "https://fixture.test/mcp",
  originalMcpArgs: "[]",
  originalMcpEnv: "{}",
  hasMcpChanges: true,
  setEditingMcp: vi.fn(),
  setEditCommand: vi.fn(),
  setEditArgs: vi.fn(),
  setEditEnv: vi.fn(),
  handleSaveMcpConfig: vi.fn(),
});

it("uses the shared remote editor and restores the original draft on revert", () => {
  setLocale("en");
  const initial = props();
  render(<McpEditView {...initial} />);
  expect(screen.getByRole("textbox", { name: "Server URL" }).getAttribute("data-slot")).toBe("input");
  expect(screen.getByRole("textbox", { name: "Headers" })).toBeTruthy();
  expect(screen.queryByRole("textbox", { name: "Arguments" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Revert changes" }));
  expect(initial.setEditCommand).toHaveBeenCalledWith(initial.originalMcpCommand);
});

it("locks saving and leaving until the native write completes", () => {
  setLocale("en");
  const initial = props();
  render(<McpEditView {...initial} saving />);
  for (const button of screen.getAllByRole("button")) expect((button as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("textbox", { name: "Server URL" }) as HTMLInputElement).disabled).toBe(true);
});
