import { act, createEvent, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import ProfileOrderHandle from "./ProfileOrderHandle";
import { setLocale } from "../lib/i18n";

function props(overrides: Partial<ComponentProps<typeof ProfileOrderHandle>> = {}) {
  return {
    id: "primary",
    name: "Primary",
    position: 1,
    count: 3,
    busy: false,
    onMove: vi.fn(),
    onDragStart: vi.fn(),
    onDragEnd: vi.fn(),
    ...overrides,
  };
}
beforeEach(() => setLocale("zh"));

describe("ProfileOrderHandle", () => {
  it.each([
    ["ArrowUp", "up"],
    ["ArrowDown", "down"],
    ["Home", "first"],
    ["End", "last"],
  ])("supports Alt+%s without opening a menu", (key, direction) => {
    const input = props();
    render(<ProfileOrderHandle {...input} />);
    const trigger = screen.getByRole("button", { name: "调整“Primary”的顺序" });
    fireEvent.keyDown(trigger, { key });
    expect(input.onMove).not.toHaveBeenCalled();
    fireEvent.keyDown(trigger, { key, altKey: true });
    expect(input.onMove).toHaveBeenCalledExactlyOnceWith("primary", direction);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("ignores composing keys and shortcuts with other modifiers", () => {
    const input = props();
    render(<ProfileOrderHandle {...input} />);
    const trigger = screen.getByRole("button", { name: "调整“Primary”的顺序" });
    for (const extra of [{ ctrlKey: true }, { metaKey: true }, { shiftKey: true }, { isComposing: true }]) {
      fireEvent.keyDown(trigger, { key: "ArrowDown", altKey: true, ...extra });
    }
    expect(input.onMove).not.toHaveBeenCalled();
  });

  it("skips disabled menu items, handles navigation and returns focus after a move", async () => {
    const input = props({ position: 0 });
    const { rerender } = render(<ProfileOrderHandle {...input} />);
    const trigger = screen.getByRole("button", { name: "调整“Primary”的顺序" });
    const scrollIntoView = vi.fn();
    trigger.scrollIntoView = scrollIntoView;
    fireEvent.click(trigger);
    const down = await screen.findByRole("menuitem", { name: "下移" });
    const last = screen.getByRole("menuitem", { name: "移到最后" });
    expect((screen.getByRole("menuitem", { name: "上移" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("menuitem", { name: "移到最前" }) as HTMLButtonElement).disabled).toBe(true);
    await waitFor(() => expect(document.activeElement).toBe(down));
    fireEvent.keyDown(down, { key: "ArrowUp" });
    expect(document.activeElement).toBe(last);
    fireEvent.keyDown(last, { key: "Home" });
    expect(document.activeElement).toBe(down);
    fireEvent.keyDown(down, { key: "End" });
    expect(document.activeElement).toBe(last);
    fireEvent.click(last);
    expect(input.onMove).toHaveBeenCalledWith("primary", "last");
    rerender(<ProfileOrderHandle {...input} position={2} busy />);
    await waitFor(() => expect(document.activeElement).toBe(trigger));
    expect(scrollIntoView).toHaveBeenCalledExactlyOnceWith({ block: "nearest", inline: "nearest" });
    expect(trigger.getAttribute("aria-disabled")).toBe("true");
    expect((trigger as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("keeps a focused trigger usable for Escape while blocking writes and dragging during save", async () => {
    const input = props();
    const { rerender } = render(<ProfileOrderHandle {...input} />);
    const trigger = screen.getByRole("button", { name: "调整“Primary”的顺序" });
    act(() => trigger.focus());
    rerender(<ProfileOrderHandle {...input} busy />);
    fireEvent.click(trigger);
    fireEvent.keyDown(trigger, { key: "ArrowDown", altKey: true });
    const drag = createEvent.dragStart(trigger, { cancelable: true });
    fireEvent(trigger, drag);
    expect(drag.defaultPrevented).toBe(true);
    expect(input.onMove).not.toHaveBeenCalled();
    expect(input.onDragStart).not.toHaveBeenCalled();
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(trigger);
    expect(trigger.draggable).toBe(false);
  });

  it("uses only the handle for drag payload and emits drag end once", () => {
    const input = props();
    render(<ProfileOrderHandle {...input} />);
    const trigger = screen.getByRole("button", { name: "调整“Primary”的顺序" });
    const transfer = { effectAllowed: "none", setData: vi.fn() };
    fireEvent.dragStart(trigger, { dataTransfer: transfer });
    expect(transfer.effectAllowed).toBe("move");
    expect(transfer.setData).toHaveBeenCalledWith("text/plain", "primary");
    expect(input.onDragStart).toHaveBeenCalledExactlyOnceWith("primary");
    fireEvent.dragEnd(trigger);
    expect(input.onDragEnd).toHaveBeenCalledTimes(1);
  });

  it.each([
    ["en", "Reorder “Primary”", "Move up"],
    ["ja", "「Primary」の並び順を変更", "上に移動"],
  ] as const)("localizes controls in %s", async (locale, label, action) => {
    setLocale(locale);
    const input = props();
    render(<ProfileOrderHandle {...input} />);
    fireEvent.click(screen.getByRole("button", { name: label }));
    fireEvent.click(await screen.findByRole("menuitem", { name: action }));
    expect(input.onMove).toHaveBeenCalledWith("primary", "up");
  });
});
