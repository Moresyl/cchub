import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import WizardView from "./WizardView";
import { en } from "../../lib/i18n/en";
import type { CodeEditorProps } from "../../components/CodeEditor";

vi.mock("../../components/DeferredCodeEditor", () => ({
  default: (props: CodeEditorProps) => <textarea aria-label="Preview" value={props.value} readOnly={props.readOnly} />,
}));
afterEach(cleanup);
const props = () => ({
  zh: false,
  i: en,
  wizardStep: 1,
  setWizardStep: vi.fn(),
  wizardDraft: { name: "Fixture", transport: "stdio" as const, command: "node", argsText: "", envText: "" },
  setWizardDraft: vi.fn(),
  wizardSyncableTools: [],
  wizardSyncTargets: [],
  setWizardSyncTargets: vi.fn(),
  wizardValidation: { isValid: true, errors: [], warnings: [], parsedArgs: [], parsedEnv: {} },
  wizardInstalling: false,
  applyWizardPreset: vi.fn(),
  closeWizard: vi.fn(),
  handleWizardInstall: vi.fn(),
});

it("provides shared named inputs and buttons without promising an automatic health check", () => {
  render(<WizardView {...props()} />);
  for (const name of ["Server Name", "Command"])
    expect(screen.getByRole("textbox", { name }).getAttribute("data-slot")).toBe("input");
  expect(screen.getByRole("textbox", { name: "Arguments" })).toBeTruthy();
  for (const button of screen.getAllByRole("button")) expect(button.getAttribute("data-slot")).toBe("button");
  expect(screen.queryByText(/health check/i)).toBeNull();
});

it.each([1, 2, 3])("locks every interactive control while installing, including step %s navigation", (wizardStep) => {
  const view = render(<WizardView {...props()} wizardStep={wizardStep} wizardInstalling />);
  for (const element of view.container.querySelectorAll("button,input,textarea"))
    expect(element.matches(":disabled")).toBe(true);
});

it("labels remote fields as server URL and headers", () => {
  const initial = props();
  render(
    <WizardView
      {...initial}
      wizardDraft={{ ...initial.wizardDraft, transport: "http", command: "https://fixture.test/mcp" }}
    />,
  );
  expect(screen.getByRole("textbox", { name: "Server URL" })).toBeTruthy();
  expect(screen.getByRole("textbox", { name: "Headers" })).toBeTruthy();
  expect(screen.queryByRole("textbox", { name: "Arguments" })).toBeNull();
});
