import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import ProfileActionsMenu from "./ProfileActionsMenu";

beforeAll(() => {
  Object.defineProperty(globalThis, "ResizeObserver", {
    configurable: true,
    value: class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  });
});

function renderMenu(overrides: Partial<React.ComponentProps<typeof ProfileActionsMenu>> = {}) {
  const props: React.ComponentProps<typeof ProfileActionsMenu> = {
    pingLabel: "Ping",
    streamLabel: "Stream",
    usageLabel: "Usage",
    duplicateLabel: "Duplicate",
    deleteLabel: "Delete",
    moreLabel: "More",
    isPinging: false,
    isStreamChecking: false,
    onPing: vi.fn(),
    onStreamCheck: vi.fn(),
    onUsage: vi.fn(),
    onDuplicate: vi.fn(),
    onDelete: vi.fn(),
    ...overrides,
  };
  render(<ProfileActionsMenu {...props} />);
  return props;
}

describe("ProfileActionsMenu", () => {
  it("opens the menu and closes it after running an action", () => {
    const props = renderMenu();

    fireEvent.click(screen.getByRole("button", { name: "More" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Usage" }));

    expect(props.onUsage).toHaveBeenCalledOnce();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("disables checks while they are already running", () => {
    renderMenu({ isPinging: true, isStreamChecking: true });
    fireEvent.click(screen.getByRole("button", { name: "More" }));

    expect(screen.getByRole("menuitem", { name: "Ping" }).getAttribute("disabled")).not.toBeNull();
    expect(screen.getByRole("menuitem", { name: "Stream" }).getAttribute("disabled")).not.toBeNull();
  });
});
