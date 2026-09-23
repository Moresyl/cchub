import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CheckboxField } from "./checkbox-field";

describe("CheckboxField", () => {
  it("exposes its label and reports checked state", () => {
    const onCheckedChange = vi.fn();
    render(
      <CheckboxField
        checked={false}
        onCheckedChange={onCheckedChange}
        label="High effort"
        description="Use deeper reasoning"
      />,
    );

    const checkbox = screen.getByRole("checkbox", { name: "High effort" });
    expect(checkbox.getAttribute("data-state")).toBe("unchecked");
    fireEvent.click(checkbox);
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });
});
