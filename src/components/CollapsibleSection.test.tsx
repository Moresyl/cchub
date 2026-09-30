import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import CollapsibleSection from "./CollapsibleSection";

describe("CollapsibleSection", () => {
  it("keeps optional fields out of the tab order until expanded", () => {
    render(
      <CollapsibleSection title="Request settings" summary="Headers and overrides">
        <input aria-label="Header value" />
      </CollapsibleSection>,
    );
    const toggle = screen.getByRole("button", { name: "Request settings" });
    expect(document.getElementById(toggle.getAttribute("aria-describedby")!)?.textContent).toBe(
      "Headers and overrides",
    );
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("textbox", { name: "Header value" })).toBeNull();
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(document.getElementById(toggle.getAttribute("aria-controls")!)?.contains(screen.getByRole("textbox"))).toBe(
      true,
    );
    fireEvent.click(toggle);
    expect(screen.queryByRole("textbox")).toBeNull();
  });

  it("can expose previously configured advanced settings immediately", () => {
    render(
      <CollapsibleSection title="Request settings" defaultOpen>
        <input aria-label="Header value" defaultValue="saved" />
      </CollapsibleSection>,
    );
    expect(screen.getByRole("button").getAttribute("aria-expanded")).toBe("true");
    expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe("saved");
  });
});
