import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import AppUpdateHost, { requestAppUpdateDialog, useAppUpdate } from "./AppUpdateHost";
import type AppUpdateDialog from "./AppUpdateDialog";

const { check, install } = vi.hoisted(() => ({ check: vi.fn(), install: vi.fn() }));
vi.mock("../lib/appUpdater", () => ({ checkAppUpdate: check, installAppUpdate: install }));
vi.mock("../lib/idleTask", () => ({ scheduleIdleTask: () => () => undefined }));
vi.mock("./AppUpdateDialog", () => ({
  default: (props: ComponentProps<typeof AppUpdateDialog>) => (
    <div>
      <span data-testid="error">{props.error}</span>
      <span data-testid="latest">{props.update?.latest_version}</span>
      <span data-testid="checking">{String(props.checking)}</span>
      <span data-testid="installing">{String(props.installing)}</span>
      <button onClick={props.onCheck}>Check</button>
      <button
        onClick={() => {
          props.onInstall();
          props.onInstall();
        }}
      >
        Install twice
      </button>
    </div>
  ),
}));

const available = {
  result: {
    update_available: true,
    latest_version: "2.0.0",
    current_version: "1.6.10",
    body: "notes",
    not_configured: false,
    can_install: true,
    release_url: null,
    source: "tauri",
    disabled_by_env: false,
  },
  handle: { source: "tauri", update: {} },
};
function State() {
  const context = useAppUpdate();
  return <span data-testid="available">{String(context.updateAvailable)}</span>;
}
function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
async function mount() {
  render(
    <AppUpdateHost>
      <State />
    </AppUpdateHost>,
  );
  act(() => requestAppUpdateDialog());
  await screen.findByText("Install twice");
  await waitFor(() => expect(screen.getByTestId("checking").textContent).toBe("false"));
}
beforeEach(() => {
  check.mockReset().mockResolvedValue(available);
  install.mockReset().mockResolvedValue(undefined);
});

describe("AppUpdateHost ownership", () => {
  it("allows only one installation and blocks checks until installation finishes", async () => {
    const pending = deferred();
    install.mockReturnValue(pending.promise);
    await mount();
    fireEvent.click(screen.getByText("Install twice"));
    expect(install).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("installing").textContent).toBe("true");
    fireEvent.click(screen.getByText("Check"));
    act(() => requestAppUpdateDialog());
    expect(check).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve());
    expect(screen.getByTestId("installing").textContent).toBe("false");
    fireEvent.click(screen.getByText("Check"));
    await waitFor(() => expect(check).toHaveBeenCalledTimes(2));
  });
  it("clears an obsolete install target after a failed recheck and permits a successful retry", async () => {
    await mount();
    expect(screen.getByTestId("available").textContent).toBe("true");
    check.mockRejectedValueOnce(new Error("offline"));
    fireEvent.click(screen.getByText("Check"));
    await waitFor(() => expect(screen.getByTestId("error").textContent).toContain("offline"));
    expect(screen.getByTestId("latest").textContent).toBe("");
    expect(screen.getByTestId("available").textContent).toBe("false");
    fireEvent.click(screen.getByText("Install twice"));
    expect(install).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Check"));
    await waitFor(() => expect(screen.getByTestId("latest").textContent).toBe("2.0.0"));
    expect(screen.getByTestId("error").textContent).toBe("");
  });
  it("blocks installing during a check and releases the install guard after failure", async () => {
    await mount();
    let finish!: (value: typeof available) => void;
    check.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    fireEvent.click(screen.getByText("Check"));
    fireEvent.click(screen.getByText("Install twice"));
    expect(install).not.toHaveBeenCalled();
    await act(async () => finish(available));
    install.mockRejectedValueOnce(new Error("download failed"));
    fireEvent.click(screen.getByText("Install twice"));
    await waitFor(() => expect(screen.getByTestId("error").textContent).toContain("download failed"));
    expect(screen.getByTestId("installing").textContent).toBe("false");
    fireEvent.click(screen.getByText("Install twice"));
    await waitFor(() => expect(install).toHaveBeenCalledTimes(2));
  });
});
