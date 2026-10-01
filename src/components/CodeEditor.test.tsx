import { afterEach, describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { EditorView } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { diagnosticCount, forceLinting } from "@codemirror/lint";
import CodeEditor, { getEditorCspNonce } from "./CodeEditor";

const addedStyles: HTMLStyleElement[] = [];

afterEach(() => {
  for (const style of addedStyles) style.remove();
  addedStyles.length = 0;
});

describe("bounded configuration editor", () => {
  it("shows real TOML diagnostics and clears them when raw syntax is corrected", async () => {
    const mounted = render(<CodeEditor language="toml" value='model = "unfinished' />);
    const content = screen.getByRole("textbox", { name: "TOML configuration editor" });
    const view = EditorView.findFromDOM(content)!;
    expect(screen.getByRole("status").getAttribute("aria-label")).toContain("TOML (1:");
    expect(screen.getByRole("status").style.background).toBe("var(--danger)");
    forceLinting(view);
    await waitFor(() => expect(diagnosticCount(view.state)).toBe(1));
    mounted.rerender(<CodeEditor language="toml" value='model = "corrected"' />);
    forceLinting(view);
    await waitFor(() => expect(diagnosticCount(view.state)).toBe(0));
    expect(screen.getByRole("status").style.background).toBe("var(--success)");
    mounted.unmount();
  });
  it("keeps long JSON scrollable inside its maximum height", () => {
    const mounted = render(
      <CodeEditor
        value={JSON.stringify({ rows: Array.from({ length: 40 }, (_, id) => ({ id })) }, null, 2)}
        minHeight={160}
        maxHeight={300}
        readOnly
      />,
    );
    const content = screen.getByRole("textbox", { name: "JSON configuration editor" });
    const view = EditorView.findFromDOM(content)!;
    expect(getComputedStyle(view.scrollDOM).maxHeight).toBe("266px");
    expect(getComputedStyle(view.scrollDOM).minHeight).toBe("126px");
    expect(content.getAttribute("aria-readonly")).toBe("true");
    expect(view.state.facet(EditorState.readOnly)).toBe(true);
    mounted.unmount();
  });

  it("updates externally supplied JSON and switches read-only semantics", () => {
    const mounted = render(<CodeEditor value='{"balance": 1}' readOnly />);
    const content = screen.getByRole("textbox", { name: "JSON configuration editor" });
    expect(EditorView.findFromDOM(content)!.state.doc.toString()).toBe('{"balance": 1}');
    mounted.rerender(<CodeEditor value='{"balance": 2}' readOnly={false} />);
    const next = screen.getByRole("textbox", { name: "JSON configuration editor" });
    const view = EditorView.findFromDOM(next)!;
    expect(view.state.doc.toString()).toBe('{"balance": 2}');
    expect(view.state.facet(EditorState.readOnly)).toBe(false);
    expect(next.getAttribute("aria-readonly")).toBe("false");
    mounted.unmount();
  });
});

describe("getEditorCspNonce", () => {
  it("uses the nonce on a style tag supplied by the host", () => {
    const style = document.createElement("style");
    style.nonce = "tauri-style-nonce";
    document.head.appendChild(style);
    addedStyles.push(style);

    expect(getEditorCspNonce()).toBe("tauri-style-nonce");
  });

  it("returns an empty nonce in an ordinary browser", () => {
    expect(getEditorCspNonce()).toBe("");
  });
});
