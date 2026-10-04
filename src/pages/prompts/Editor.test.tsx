import { useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import PromptEditor from "./Editor";
import { emptyDraft } from "./types";

vi.mock("../../components/CodeEditor", () => ({ default: () => <div /> }));
vi.mock("../../components/MarkdownPreview", () => ({ default: () => <div /> }));
afterEach(cleanup);

it("explains the required name without marking an untouched draft invalid, and enables saving after correction", () => {
  const save = vi.fn();
  function Harness() {
    const [draft, setDraft] = useState(emptyDraft);
    return (
      <PromptEditor
        draft={draft}
        onChange={setDraft}
        onSave={save}
        onClose={() => {}}
        writing={false}
        blocked={false}
        canWriteLive
        text={(_zh, en) => en}
      />
    );
  }
  render(<Harness />);
  const name = screen.getAllByRole("textbox")[0];
  const button = screen.getByRole("button", { name: "Save" }) as HTMLButtonElement;
  expect(name.getAttribute("aria-invalid")).toBeNull();
  expect(document.getElementById(name.getAttribute("aria-describedby")!)?.textContent).toContain("Required");
  expect(button.disabled).toBe(true);
  fireEvent.blur(name);
  expect(name.getAttribute("aria-invalid")).toBe("true");
  fireEvent.change(name, { target: { value: "Project instructions" } });
  expect(name.getAttribute("aria-invalid")).toBeNull();
  expect(button.disabled).toBe(false);
  fireEvent.click(button);
  expect(save).toHaveBeenCalledWith(false);
  fireEvent.change(name, { target: { value: "x".repeat(121) } });
  expect(button.disabled).toBe(true);
});
