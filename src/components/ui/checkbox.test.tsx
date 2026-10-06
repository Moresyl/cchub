import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Checkbox } from "./checkbox";
import { CheckboxField } from "./checkbox-field";

afterEach(cleanup);

describe("Checkbox", () => {
  it("moves an uncontrolled mixed selection to checked on click", () => {
    const onCheckedChange = vi.fn();
    render(<Checkbox defaultChecked="indeterminate" onCheckedChange={onCheckedChange} aria-label="Select all" />);

    const checkbox = screen.getByRole("checkbox", { name: "Select all" });
    expect(checkbox.getAttribute("aria-checked")).toBe("mixed");
    fireEvent.click(checkbox);
    expect(onCheckedChange).toHaveBeenCalledWith(true);
    expect(checkbox.getAttribute("aria-checked")).toBe("true");
    expect(checkbox.getAttribute("data-state")).toBe("checked");
  });

  it("reflects controlled changes without retaining a stale mixed state", () => {
    const { rerender } = render(<Checkbox checked="indeterminate" aria-label="Selection" />);
    expect(screen.getByRole("checkbox").getAttribute("aria-checked")).toBe("mixed");
    rerender(<Checkbox checked={false} aria-label="Selection" />);
    expect(screen.getByRole("checkbox").getAttribute("aria-checked")).toBe("false");
    rerender(<Checkbox checked aria-label="Selection" />);
    expect(screen.getByRole("checkbox").getAttribute("aria-checked")).toBe("true");
  });

  it("keeps a disabled mixed selection unchanged", () => {
    const onCheckedChange = vi.fn();
    render(
      <Checkbox defaultChecked="indeterminate" disabled onCheckedChange={onCheckedChange} aria-label="Selection" />,
    );
    const checkbox = screen.getByRole("checkbox");
    fireEvent.click(checkbox);
    expect(onCheckedChange).not.toHaveBeenCalled();
    expect(checkbox.getAttribute("aria-checked")).toBe("mixed");
  });
});

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

  it("toggles through the label and exposes its description", () => {
    const onCheckedChange = vi.fn();
    render(
      <CheckboxField
        checked={false}
        onCheckedChange={onCheckedChange}
        label="Backups"
        description="Before switching"
      />,
    );
    const checkbox = screen.getByRole("checkbox", { name: "Backups" });
    expect(document.getElementById(checkbox.getAttribute("aria-describedby")!)?.textContent).toBe("Before switching");
    fireEvent.click(screen.getByText("Backups"));
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it("does not toggle a disabled field through its label", () => {
    const onCheckedChange = vi.fn();
    render(<CheckboxField checked disabled onCheckedChange={onCheckedChange} label="Backups" />);
    fireEvent.click(screen.getByText("Backups"));
    expect(onCheckedChange).not.toHaveBeenCalled();
    expect(screen.getByRole("checkbox").getAttribute("aria-checked")).toBe("true");
  });
});
