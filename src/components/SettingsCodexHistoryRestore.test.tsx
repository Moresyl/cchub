import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SettingsCodexHistorySection from "./SettingsCodexHistorySection";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const backup = {
  key: "generation",
  createdAtSeconds: 1700000000,
  targetProviderId: "custom",
  logFiles: 3,
  stateRows: 2,
  problem: null,
};
const item = {
  key: "a",
  sessionId: "session-a",
  originalProviderId: "legacy",
  status: "ready",
  reason: null,
  logFiles: 2,
  stateRows: 1,
};
const preview = {
  backupKey: "generation",
  revision: "checked",
  items: [
    item,
    { ...item, key: "b", sessionId: "session-b", status: "conflict", reason: "当前分桶已由其他操作修改" },
    { ...item, key: "c", sessionId: "session-c", status: "restored", logFiles: 0, stateRows: 0 },
  ],
};
const result = { restoredJsonlFiles: 2, restoredStateRows: 1, backupPath: "C:/fixture/safety-backup" };

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
  fireEvent.click(screen.getByRole("button", { name: "恢复迁移备份" }));
  await screen.findByRole("combobox", { name: "选择迁移备份" });
  fireEvent.click(screen.getByRole("button", { name: "预览恢复范围" }));
  await screen.findByText("恢复预览");
}
afterEach(cleanup);
beforeEach(() => invoke.mockReset());

describe("迁移备份选择恢复", () => {
  it("默认仅选择可恢复会话，冲突与已恢复项禁用，取消不会写入", async () => {
    invoke.mockResolvedValueOnce([backup]).mockResolvedValueOnce(preview);
    const { invalidate } = mount();
    await check();
    expect(screen.getByText("已选 1 个会话 · 2 个日志 · 1 条状态")).toBeTruthy();
    expect(screen.getByRole("checkbox", { name: "恢复会话 session-a 到 legacy" }).getAttribute("data-state")).toBe(
      "checked",
    );
    expect(screen.getByRole("checkbox", { name: "恢复会话 session-b 到 legacy" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("checkbox", { name: "恢复会话 session-c 到 legacy" }).hasAttribute("disabled")).toBe(true);
    const migration = screen.getByRole("button", { name: "检查历史" });
    fireEvent.click(screen.getByRole("button", { name: "恢复所选会话" }));
    await screen.findByRole("dialog");
    expect(migration.hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(invoke.mock.calls).toEqual([
      ["list_codex_history_migration_backups", {}],
      ["preview_codex_history_restore", { backupKey: "generation" }],
    ]);
    expect(invalidate).not.toHaveBeenCalled();
  });

  it("绑定备份和预览版本执行，忙碌时阻止重复操作与关闭，成功清除预览", async () => {
    let finish!: (value: typeof result) => void;
    invoke
      .mockResolvedValueOnce([backup])
      .mockResolvedValueOnce(preview)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      );
    const { invalidate } = mount();
    await check();
    const migration = screen.getByRole("button", { name: "检查历史" });
    fireEvent.click(screen.getByRole("button", { name: "恢复所选会话" }));
    const confirm = await screen.findByRole("button", { name: "创建备份并恢复" });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.getByRole("dialog").textContent).toContain("将恢复 1 个会话");
    expect(migration.hasAttribute("disabled")).toBe(true);
    expect(invoke).toHaveBeenLastCalledWith("restore_codex_history_migration", {
      backupKey: "generation",
      expectedRevision: "checked",
      selectedKeys: ["a"],
    });
    expect(invoke).toHaveBeenCalledTimes(3);
    await act(async () => finish(result));
    await screen.findByText("已恢复 2 个日志、1 条状态记录");
    expect(screen.queryByText("恢复预览")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["sessions"] });
    expect(screen.getByRole("button", { name: "检查历史" }).hasAttribute("disabled")).toBe(false);
  });

  it("失效预览要求重新检查，错误后的重试使用新版本", async () => {
    invoke
      .mockResolvedValueOnce([backup])
      .mockResolvedValueOnce(preview)
      .mockRejectedValueOnce("历史在恢复预览后发生变化，请重新检查")
      .mockResolvedValueOnce({ ...preview, revision: "fresh" })
      .mockResolvedValueOnce(result);
    const { invalidate } = mount();
    await check();
    fireEvent.click(screen.getByRole("button", { name: "恢复所选会话" }));
    fireEvent.click(await screen.findByRole("button", { name: "创建备份并恢复" }));
    expect((await screen.findByRole("alert")).textContent).toContain("预览后发生变化");
    expect(screen.queryByRole("button", { name: "恢复所选会话" })).toBeNull();
    expect(invalidate).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "预览恢复范围" }));
    await screen.findByText("恢复预览");
    fireEvent.click(screen.getByRole("button", { name: "恢复所选会话" }));
    fireEvent.click(await screen.findByRole("button", { name: "创建备份并恢复" }));
    await screen.findByText("已恢复 2 个日志、1 条状态记录");
    expect(invoke).toHaveBeenLastCalledWith("restore_codex_history_migration", {
      backupKey: "generation",
      expectedRevision: "fresh",
      selectedKeys: ["a"],
    });
  });

  it("多页会话搜索保留选择，全不选阻止空恢复", async () => {
    const items = Array.from({ length: 45 }, (_, index) => ({
      ...item,
      key: String(index),
      sessionId: `session-${index}`,
    }));
    invoke.mockResolvedValueOnce([backup]).mockResolvedValueOnce({ ...preview, items });
    mount();
    await check();
    const list = screen.getByRole("list", { name: "恢复会话列表" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(40);
    fireEvent.click(screen.getByRole("button", { name: "下一页" }));
    expect(within(list).getAllByRole("listitem")).toHaveLength(5);
    fireEvent.click(screen.getByRole("checkbox", { name: "恢复会话 session-44 到 legacy" }));
    fireEvent.change(screen.getByRole("textbox", { name: "搜索恢复会话" }), { target: { value: "session-44" } });
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);
    expect(screen.getByText("已选 44 个会话 · 88 个日志 · 44 条状态")).toBeTruthy();
    fireEvent.click(screen.getByRole("checkbox", { name: "选择全部可恢复会话" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "选择全部可恢复会话" }));
    expect(screen.getByRole("button", { name: "恢复所选会话" }).hasAttribute("disabled")).toBe(true);
  });

  it("空备份、坏账本和迟到的请求都不会产生错误执行入口", async () => {
    invoke.mockResolvedValueOnce([]);
    const { unmount } = mount();
    fireEvent.click(screen.getByRole("button", { name: "恢复迁移备份" }));
    await screen.findByText("当前配置目录没有迁移备份。");
    expect(screen.queryByRole("button", { name: "预览恢复范围" })).toBeNull();
    unmount();
    invoke.mockResolvedValueOnce([{ ...backup, problem: "历史备份账本损坏" }]);
    const second = mount();
    fireEvent.click(screen.getByRole("button", { name: "恢复迁移备份" }));
    await screen.findByText("有一份备份无法使用：历史备份账本损坏");
    expect(screen.getByRole("button", { name: "预览恢复范围" }).hasAttribute("disabled")).toBe(true);
    second.unmount();
    let finish!: (value: (typeof backup)[]) => void;
    invoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const third = mount();
    fireEvent.click(screen.getByRole("button", { name: "恢复迁移备份" }));
    third.unmount();
    await act(async () => finish([backup]));
    expect(screen.queryByText("选择迁移备份")).toBeNull();
  });
});
