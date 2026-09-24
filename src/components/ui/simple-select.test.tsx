import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SimpleSelect } from "./simple-select";

describe("SimpleSelect", () => {
  it("renders the selected option through the shared select trigger", () => {
    render(
      <SimpleSelect
        value="claude"
        ariaLabel="Target app"
        options={[
          { value: "claude", label: "Claude" },
          { value: "codex", label: "Codex" },
        ]}
        onValueChange={vi.fn()}
      />,
    );

    expect(screen.getByRole("combobox", { name: "Target app" }).textContent).toContain("Claude");
  });

  it("supports an empty option without passing an empty item value to Radix", () => {
    render(
      <SimpleSelect
        value=""
        onValueChange={() => undefined}
        options={[
          { value: "", label: "Default" },
          { value: "custom", label: "Custom" },
        ]}
      />,
    );

    expect(screen.getByRole("combobox").textContent).to.contain("Default");
  });
});
