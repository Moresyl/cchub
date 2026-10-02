import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setLocale } from "../../lib/i18n";
import AdmissionPanel, { DEFAULT_ADMISSION } from "./AdmissionPanel";
import type { AdmissionConfig } from "./types";

const { invoke, change } = vi.hoisted(() => ({ invoke: vi.fn(), change: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const key = "a".repeat(64);
const status = {
  active: 1,
  queued: 2,
  entries: [{ key, profiles: ["工作账户", "同账户备用配置"], active: 1, queued: 2, limit: 2 }],
};

function Host({ initial = DEFAULT_ADMISSION, disabled = false }: { initial?: AdmissionConfig; disabled?: boolean }) {
  const [config, setConfig] = useState(initial);
  return (
    <fieldset disabled={disabled}>
      <AdmissionPanel
        config={config}
        onChange={(value) => {
          change(value);
          setConfig(value);
        }}
      />
    </fieldset>
  );
}
beforeEach(() => {
  setLocale("zh");
  vi.clearAllMocks();
  invoke.mockResolvedValue(status);
});
afterEach(cleanup);

describe("account admission controls", () => {
  it("uses shared control sizes and edits bounded defaults without fetching account credentials", () => {
    render(<Host />);
    const limit = screen.getByRole("spinbutton", { name: "每个账户的默认并发" });
    expect(limit.getAttribute("data-control-size")).toBe("md");
    fireEvent.change(limit, { target: { value: "3" } });
    expect(change).toHaveBeenLastCalledWith(expect.objectContaining({ maxConcurrent: 3 }));
    const wait = screen.getByRole("spinbutton", { name: "最长排队时间（秒）" });
    fireEvent.change(wait, { target: { value: "0" } });
    expect(wait.getAttribute("aria-invalid")).toBe("true");
    fireEvent.blur(wait);
    expect(change).toHaveBeenLastCalledWith(expect.objectContaining({ queueTimeoutSecs: 1 }));
    expect(invoke).not.toHaveBeenCalled();
  });

  it("edits one shared account override and explicitly restores inheritance", async () => {
    render(<Host initial={{ ...DEFAULT_ADMISSION, maxConcurrent: 2 }} />);
    fireEvent.click(screen.getByRole("button", { name: "查看账户并发" }));
    const input = await screen.findByRole("spinbutton", { name: "工作账户 / 同账户备用配置" });
    expect(invoke).toHaveBeenCalledWith("get_proxy_admission_stats");
    fireEvent.change(input, { target: { value: "0" } });
    expect(screen.getByText("当前生效上限: 2")).toBeTruthy();
    expect(change).toHaveBeenLastCalledWith(
      expect.objectContaining({ accountLimits: { [key]: 0 }, accountLabels: { [key]: "工作账户 / 同账户备用配置" } }),
    );
    fireEvent.click(screen.getByRole("button", { name: "恢复默认并发" }));
    expect(change).toHaveBeenLastCalledWith(expect.objectContaining({ accountLimits: {}, accountLabels: {} }));
    await waitFor(() => expect((input as HTMLInputElement).value).toBe("2"));
    expect(screen.queryByText(key)).toBeNull();
  });

  it("keeps saved account overrides visible after their requests finish", async () => {
    invoke.mockResolvedValue({ active: 0, queued: 0, entries: [] });
    render(
      <Host
        initial={{ ...DEFAULT_ADMISSION, accountLimits: { [key]: 4 }, accountLabels: { [key]: "已保存工作账户" } }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "查看账户并发" }));
    const input = await screen.findByRole("spinbutton", { name: "已保存工作账户" });
    expect((input as HTMLInputElement).value).toBe("4");
  });

  it("preserves readings and draft overrides after refresh failure without exposing private errors", async () => {
    render(<Host />);
    fireEvent.click(screen.getByRole("button", { name: "查看账户并发" }));
    const input = await screen.findByRole("spinbutton", { name: "工作账户 / 同账户备用配置" });
    fireEvent.change(input, { target: { value: "4" } });
    invoke.mockRejectedValueOnce(new Error("private-credential-sentinel"));
    fireEvent.click(screen.getByRole("button", { name: "刷新状态" }));
    expect((await screen.findByRole("alert")).textContent).toContain("未能更新状态");
    expect((input as HTMLInputElement).value).toBe("4");
    expect(document.body.textContent).not.toContain("private-credential-sentinel");
    fireEvent.click(screen.getByRole("button", { name: "刷新状态" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("ignores an older refresh that completes after a newer expansion", async () => {
    let resolve!: (value: typeof status) => void;
    invoke.mockReturnValueOnce(
      new Promise((yes) => {
        resolve = yes;
      }),
    );
    render(<Host />);
    const toggle = screen.getByRole("button", { name: "查看账户并发" });
    fireEvent.click(toggle);
    fireEvent.click(toggle);
    fireEvent.click(toggle);
    await screen.findByRole("spinbutton", { name: "工作账户 / 同账户备用配置" });
    resolve({ active: 0, queued: 0, entries: [] });
    await waitFor(() => expect(screen.getByRole("spinbutton", { name: "工作账户 / 同账户备用配置" })).toBeTruthy());
  });

  it("honors the parent saving and read-error lock for keyboard and pointer controls", () => {
    render(<Host disabled />);
    for (const input of screen.getAllByRole("spinbutton")) expect(input.matches(":disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "查看账户并发" }).matches(":disabled")).toBe(true);
  });
});
