import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { en } from "../../lib/i18n/en";
import { showToast } from "../../components/Toast";
import type { CodeEditorProps } from "../../components/CodeEditor";
import EditView from "./EditView";

vi.mock("../../components/Toast", () => ({ showToast: vi.fn() }));
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
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it("locks draft fields and exit actions while the save is pending", () => {
  const initial = props();
  render(<EditView {...initial} saving />);
  for (const button of screen.getAllByRole("button")) {
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
  }
  expect((screen.getByRole("textbox", { name: "Command" }) as HTMLInputElement).disabled).toBe(true);
  for (const name of ["Arguments", "Environment"]) {
    expect((screen.getByRole("textbox", { name }) as HTMLTextAreaElement).readOnly).toBe(true);
  }
  expect(initial.handleSave).not.toHaveBeenCalled();
  expect(initial.setEditing).not.toHaveBeenCalled();
});
const props = () => ({
  selected: {
    id: "one",
    name: "Fixture",
    command: "node",
    args: "[]",
    env: "{}",
    status: "active",
    transport: "stdio",
    source: "local",
    package_name: null,
    version: null,
    config_path: null,
  },
  i: en,
  zh: false,
  editCommand: "node",
  setEditCommand: vi.fn(),
  editArgs: '["server.js"]',
  setEditArgs: vi.fn(),
  editEnv: '{"MODE":"test"}',
  setEditEnv: vi.fn(),
  setEditing: vi.fn(),
  handleSave: vi.fn(),
});

it("labels the fields and provides shared formatting, save and cancel controls", () => {
  const initial = props();
  render(<EditView {...initial} />);
  const command = screen.getByRole("textbox", { name: "Command" });
  expect(command.getAttribute("data-slot")).toBe("input");
  fireEvent.change(command, { target: { value: "python" } });
  expect(initial.setEditCommand).toHaveBeenCalledWith("python");
  fireEvent.click(screen.getByRole("button", { name: "Format arguments" }));
  expect(initial.setEditArgs).toHaveBeenCalledWith('[\n  "server.js"\n]');
  fireEvent.click(screen.getByRole("button", { name: "Format environment" }));
  expect(initial.setEditEnv).toHaveBeenCalledWith('{\n  "MODE": "test"\n}');
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(initial.handleSave).toHaveBeenCalledTimes(1);
  for (const button of screen.getAllByRole("button", { name: "Cancel" })) {
    expect(button.getAttribute("data-slot")).toBe("button");
    fireEvent.click(button);
  }
  expect(initial.setEditing).toHaveBeenCalledTimes(2);
  expect(initial.setEditing).toHaveBeenCalledWith(false);
});

it("does not overwrite malformed drafts or disclose their contents in format errors", () => {
  const initial = props();
  render(<EditView {...initial} editArgs="private argument unfinished" editEnv='{"API_KEY":"private-secret"' />);
  fireEvent.click(screen.getByRole("button", { name: "Format arguments" }));
  fireEvent.click(screen.getByRole("button", { name: "Format environment" }));
  expect(initial.setEditArgs).not.toHaveBeenCalled();
  expect(initial.setEditEnv).not.toHaveBeenCalled();
  expect(vi.mocked(showToast).mock.calls).toEqual([
    ["error", "Invalid arguments JSON. Your content is preserved."],
    ["error", "Invalid environment JSON. Your content is preserved."],
  ]);
  expect((screen.getByRole("textbox", { name: "Environment" }) as HTMLTextAreaElement).value).toContain(
    "private-secret",
  );
});
