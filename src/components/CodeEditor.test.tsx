import { afterEach, describe, expect, it } from "vitest";
import { getEditorCspNonce } from "./CodeEditor";

const addedStyles: HTMLStyleElement[] = [];

afterEach(() => {
  for (const style of addedStyles) style.remove();
  addedStyles.length = 0;
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
