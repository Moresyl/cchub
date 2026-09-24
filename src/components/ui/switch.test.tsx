import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Switch } from "./switch";

describe("Switch", () => {
  it("reports its next checked state and exposes an accessible label", () => {
    const onCheckedChange = vi.fn();
    render(<Switch checked={false} onCheckedChange={onCheckedChange} aria-label="Automatic backups" />);

    const control = screen.getByRole("switch", { name: "Automatic backups" });
    expect(control.getAttribute("data-state")).toBe("unchecked");
    fireEvent.click(control);
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it("does not react while disabled", () => {
    const onCheckedChange = vi.fn();
    render(<Switch checked={false} disabled onCheckedChange={onCheckedChange} aria-label="Sync" />);

    fireEvent.click(screen.getByRole("switch", { name: "Sync" }));
    expect(onCheckedChange).not.toHaveBeenCalled();
  });
});
