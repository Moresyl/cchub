import { Suspense } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { CodeEditorProps } from "./CodeEditor";
import DeferredCodeEditor from "./DeferredCodeEditor";

const loader = vi.hoisted(() => {
  let resolve!: () => void;
  const ready = new Promise<void>((done) => {
    resolve = done;
  });
  return { ready, resolve, fail: false };
});
vi.mock("../lib/i18n", () => ({ getLocale: () => "en" }));
vi.mock("./CodeEditor", async () => {
  await loader.ready;
  return {
    default: (props: CodeEditorProps) => {
      if (loader.fail) throw new Error("private configuration and chunk path");
      return (
        <textarea
          aria-label={props.ariaLabel ?? "Editor"}
          value={props.value}
          readOnly={props.readOnly}
          onChange={(event) => props.onChange?.(event.target.value)}
        />
      );
    },
  };
});
afterEach(() => {
  cleanup();
  loader.fail = false;
  vi.restoreAllMocks();
});

it("reserves the field during first load, keeps surrounding controls and uses the latest draft", async () => {
  const shell = (value: string) => (
    <Suspense fallback={<p>Whole page loading</p>}>
      <button>Cancel</button>
      <DeferredCodeEditor ariaLabel="Arguments" value={value} minHeight={160} maxHeight={280} />
    </Suspense>
  );
  const view = render(shell("private initial draft"));
  expect(screen.getByRole("button", { name: "Cancel" })).toBeTruthy();
  expect(screen.queryByText("Whole page loading")).toBeNull();
  const status = screen.getByRole("status");
  expect(status.style.minHeight).toBe("160px");
  expect(status.style.maxHeight).toBe("280px");
  expect(status.textContent).not.toContain("private initial draft");
  view.rerender(shell("latest draft"));
  await act(async () => {
    loader.resolve();
    await loader.ready;
  });
  expect(((await screen.findByRole("textbox", { name: "Arguments" })) as HTMLTextAreaElement).value).toBe(
    "latest draft",
  );
  expect(screen.queryByText("Loading editor…")).toBeNull();
});

it("contains a failed editor, hides private errors and keeps the current content editable", async () => {
  vi.spyOn(console, "error").mockImplementation(() => {});
  loader.fail = true;
  const change = vi.fn();
  const view = render(
    <>
      <button>Save</button>
      <DeferredCodeEditor value="draft one" ariaLabel="Arguments" onChange={change} />
    </>,
  );
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toBe("Code highlighting is unavailable. Your original text is preserved.");
  expect(screen.queryByText(/private configuration/)).toBeNull();
  expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
  view.rerender(
    <>
      <button>Save</button>
      <DeferredCodeEditor value="draft two" ariaLabel="Arguments" onChange={change} />
    </>,
  );
  const field = screen.getByRole("textbox", { name: "Arguments" }) as HTMLTextAreaElement;
  expect(field.value).toBe("draft two");
  expect(field.getAttribute("data-slot")).toBe("textarea");
  fireEvent.change(field, { target: { value: "corrected draft" } });
  expect(change).toHaveBeenCalledWith("corrected draft");
  view.rerender(<DeferredCodeEditor value="saved" ariaLabel="Arguments" readOnly />);
  expect((screen.getByRole("textbox", { name: "Arguments" }) as HTMLTextAreaElement).readOnly).toBe(true);
});

it("forwards changes and read-only mode through the deferred field", async () => {
  const change = vi.fn();
  const view = render(<DeferredCodeEditor value="before" onChange={change} ariaLabel="Environment" />);
  const input = await screen.findByRole("textbox", { name: "Environment" });
  fireEvent.change(input, { target: { value: "after" } });
  expect(change).toHaveBeenCalledWith("after");
  view.rerender(<DeferredCodeEditor value="saved" readOnly ariaLabel="Environment" />);
  expect((screen.getByRole("textbox", { name: "Environment" }) as HTMLTextAreaElement).readOnly).toBe(true);
});
