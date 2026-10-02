import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AppUpdateDialog from "./AppUpdateDialog";
import { setLocale } from "../lib/i18n";

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
  beforeEach(() => setLocale("zh"));
  it("renders release details and starts installation", async () => {
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
    await act(() => vi.dynamicImportSettled());
    expect(await screen.findByText("Durable Pi session accounting")).toBeTruthy();
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

  it("changes release-note language immediately without checking the release channel again", async () => {
    const onCheck = vi.fn();
    render(
      <AppUpdateDialog
        isOpen
        update={{
          ...availableUpdate,
          body: "## 更新内容 / Highlights\n\n- 中文说明\n\n## English Summary\n\n- English notes",
        }}
        checking={false}
        installing={false}
        installProgress={null}
        error={null}
        onClose={vi.fn()}
        onCheck={onCheck}
        onInstall={vi.fn()}
      />,
    );
    expect(await screen.findByText("中文说明")).toBeTruthy();
    const notes = screen.getByText("中文说明").closest(".app-release-notes")!;
    notes.scrollTop = 200;
    expect(screen.queryByText("English notes")).toBeNull();
    act(() => setLocale("en"));
    expect(await screen.findByText("English notes")).toBeTruthy();
    expect(notes.scrollTop).toBe(0);
    expect(screen.queryByText("中文说明")).toBeNull();
    act(() => setLocale("ja"));
    expect(screen.getByRole("heading", { name: "更新内容" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "更新して再起動" })).toBeTruthy();
    expect(onCheck).not.toHaveBeenCalled();
  });

  it.each([
    [null, "network failed", "未能检查更新"],
    [{ ...availableUpdate, update_available: false, disabled_by_env: true }, null, "自动更新已关闭"],
    [{ ...availableUpdate, update_available: false, not_configured: true }, null, "更新渠道暂不可用"],
    [null, null, "检查更新"],
  ] as const)("does not claim a successful check for unavailable update state %j", (update, error, title) => {
    render(
      <AppUpdateDialog
        isOpen
        update={update}
        checking={false}
        installing={false}
        installProgress={null}
        error={error}
        onClose={vi.fn()}
        onCheck={vi.fn()}
        onInstall={vi.fn()}
      />,
    );
    expect(screen.getByRole("heading", { name: title })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "当前已是最新版本" })).toBeNull();
  });

  it("blocks installation while a new check is in progress", () => {
    const onInstall = vi.fn();
    render(
      <AppUpdateDialog
        isOpen
        update={availableUpdate}
        checking
        installing={false}
        installProgress={null}
        error={null}
        onClose={vi.fn()}
        onCheck={vi.fn()}
        onInstall={onInstall}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "一键更新并重启" }));
    expect(onInstall).not.toHaveBeenCalled();
  });
});
