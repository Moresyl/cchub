import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ConfirmDialog from "./ConfirmDialog";

describe("ConfirmDialog", () => {
  it("exposes an accessible modal and confirms once", () => {
    const onConfirm = vi.fn();
    render(
      <ConfirmDialog
        isOpen
        title="Delete profile"
        message="This cannot be undone"
        confirmText="Delete"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("dialog", { name: "Delete profile" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("does not render modal content while closed", () => {
    render(
      <ConfirmDialog isOpen={false} title="Hidden" message="Hidden message" onConfirm={vi.fn()} onCancel={vi.fn()} />,
    );

    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
