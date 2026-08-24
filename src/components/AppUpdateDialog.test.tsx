import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import AppUpdateDialog from "./AppUpdateDialog";

const availableUpdate = {
  current_version: "1.4.6",
  latest_version: "1.5.0",
  update_available: true,
  can_install: true,
  body: "Durable Pi session accounting",
  not_configured: false,
  release_url: null,
  source: "tauri" as const,
  disabled_by_env: false,
};

describe("AppUpdateDialog", () => {
  it("renders release details and starts installation", () => {
    const onInstall = vi.fn();
    render(
      <AppUpdateDialog
        isOpen
        update={availableUpdate}
        checking={false}
        installing={false}
        installProgress={null}
        error={null}
        onClose={vi.fn()}
        onCheck={vi.fn()}
        onInstall={onInstall}
      />,
    );

    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(screen.getByText("Durable Pi session accounting")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /更新并重启|Update and restart/ }));
    expect(onInstall).toHaveBeenCalledTimes(1);
  });

  it("locks dismissal and reports determinate install progress", () => {
    const onClose = vi.fn();
    render(
      <AppUpdateDialog
        isOpen
        update={availableUpdate}
        checking={false}
        installing
        installProgress={64}
        error={null}
        onClose={onClose}
        onCheck={vi.fn()}
        onInstall={vi.fn()}
      />,
    );

    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("64");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });
});
