import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SettingsCodexHistorySection from "./SettingsCodexHistorySection";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const preview = {
  revision: "checked-revision",
  sourceProviderIds: ["legacy"],
  targetProviderId: "custom",
  jsonlFiles: 2,
  compressedFiles: 1,
  stateRows: 3,
};
const result = {
  sourceProviderIds: ["legacy"],
  targetProviderId: "custom",
  migratedJsonlFiles: 2,
  migratedStateRows: 3,
  backupPath: "C:/fixture/history-backup",
  skippedReason: null,
};

function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const view = render(
    <QueryClientProvider client={client}>
      <SettingsCodexHistorySection />
    </QueryClientProvider>,
  );
  return { ...view, invalidate };
}
async function check() {
  fireEvent.click(screen.getByRole("button", { name: "检查历史" }));
  await screen.findByText("检查完成，等待迁移");
}
afterEach(cleanup);
beforeEach(() => invoke.mockReset());

describe("历史迁移预览", () => {
  it("只读检查显示范围，取消确认不会修改历史", async () => {
    invoke.mockResolvedValue(preview);
    const { invalidate } = mount();
    await check();
    expect(screen.getByText("2 个（压缩 1 个）")).toBeTruthy();
    expect(invoke.mock.calls).toEqual([["preview_codex_history_migration", {}]]);
    fireEvent.click(screen.getByRole("button", { name: "迁移已检查的历史" }));
    await screen.findByRole("dialog");
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invalidate).not.toHaveBeenCalled();
  });

  it("执行绑定已检查内容，等待期间不能重复提交或关闭确认", async () => {
    let finish!: (value: typeof result) => void;
    invoke.mockResolvedValueOnce(preview).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const { invalidate } = mount();
    await check();
    fireEvent.click(screen.getByRole("button", { name: "迁移已检查的历史" }));
    const confirm = await screen.findByRole("button", { name: "创建备份并迁移" });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke).toHaveBeenLastCalledWith("migrate_codex_history", {
      sourceProviderIds: ["legacy"],
      targetProviderId: "custom",
      expectedRevision: "checked-revision",
    });
    expect(screen.getByRole("button", { name: "迁移中…" }).hasAttribute("disabled")).toBe(true);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.getByRole("dialog")).toBeTruthy();
    await act(async () => finish(result));
    await screen.findByText("已迁移 2 个会话文件、3 条状态记录");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["sessions"] });
    expect(screen.getByText(/C:\/fixture\/history-backup/)).toBeTruthy();
  });

  it("过期预览错误清除执行入口，重新检查后才能重试", async () => {
    invoke
      .mockResolvedValueOnce(preview)
      .mockRejectedValueOnce("历史在预览后发生变化，请重新检查再迁移")
      .mockResolvedValueOnce({ ...preview, revision: "fresh" });
    const { invalidate } = mount();
    await check();
    fireEvent.click(screen.getByRole("button", { name: "迁移已检查的历史" }));
    fireEvent.click(await screen.findByRole("button", { name: "创建备份并迁移" }));
    expect((await screen.findByRole("alert")).textContent).toContain("预览后发生变化");
    expect(screen.queryByRole("button", { name: "迁移已检查的历史" })).toBeNull();
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["sessions"] });
    await check();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("没有变化时不给出迁移按钮，检查失败可以重试", async () => {
    invoke
      .mockRejectedValueOnce("会话最近仍在更新")
      .mockResolvedValueOnce({ ...preview, jsonlFiles: 0, compressedFiles: 0, stateRows: 0 });
    mount();
    fireEvent.click(screen.getByRole("button", { name: "检查历史" }));
    expect((await screen.findByRole("alert")).textContent).toContain("最近仍在更新");
    fireEvent.click(screen.getByRole("button", { name: "检查历史" }));
    await screen.findByText("没有需要迁移的历史");
    expect(screen.queryByRole("button", { name: "迁移已检查的历史" })).toBeNull();
    expect(invoke.mock.calls.every(([cmd]) => cmd === "preview_codex_history_migration")).toBe(true);
  });
});
