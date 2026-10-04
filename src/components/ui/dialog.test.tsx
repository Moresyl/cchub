import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Dialog, DialogContent, DialogTitle } from "./dialog";
import { Input } from "./input";

afterEach(cleanup);

async function outsideClick() {
  await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
  fireEvent.pointerDown(document.body, { button: 0, pointerType: "mouse" });
  fireEvent.pointerUp(document.body, { button: 0, pointerType: "mouse" });
  fireEvent.click(document.body);
}

it("keeps an edited draft on outside clicks but allows explicit close", async () => {
  const close = vi.fn();
  const outside = vi.fn();
  render(
    <Dialog defaultOpen onOpenChange={close}>
      <DialogContent aria-describedby={undefined} onInteractOutside={outside}>
        <DialogTitle>Configuration</DialogTitle>
        <Input aria-label="Name" defaultValue="original" />
      </DialogContent>
    </Dialog>,
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "draft" } });
  await outsideClick();
  expect(outside).toHaveBeenCalledTimes(1);
  expect(close).not.toHaveBeenCalled();
  expect((screen.getByRole("textbox", { name: "Name" }) as HTMLInputElement).value).toBe("draft");
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(close).toHaveBeenCalledWith(false);
});

it.each([false, true])(
  "lets read-only dialogs opt into outside dismissal while respecting caller prevention: %s",
  async (prevent) => {
    const close = vi.fn();
    render(
      <Dialog defaultOpen onOpenChange={close}>
        <DialogContent
          aria-describedby={undefined}
          dismissOnOutsideClick
          onInteractOutside={(event) => {
            if (prevent) event.preventDefault();
          }}
        >
          <DialogTitle>Preview</DialogTitle>
        </DialogContent>
      </Dialog>,
    );
    await outsideClick();
    if (prevent) expect(close).not.toHaveBeenCalled();
    else expect(close).toHaveBeenCalledWith(false);
  },
);

it("preserves Escape and caller-controlled pending-operation protection", () => {
  const close = vi.fn();
  const view = render(
    <Dialog defaultOpen onOpenChange={close}>
      <DialogContent aria-describedby={undefined} onEscapeKeyDown={(event) => event.preventDefault()}>
        <DialogTitle>Saving</DialogTitle>
      </DialogContent>
    </Dialog>,
  );
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(close).not.toHaveBeenCalled();
  view.rerender(
    <Dialog defaultOpen onOpenChange={close}>
      <DialogContent aria-describedby={undefined}>
        <DialogTitle>Saved</DialogTitle>
      </DialogContent>
    </Dialog>,
  );
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(close).toHaveBeenCalledWith(false);
});
