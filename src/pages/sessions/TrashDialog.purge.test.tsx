import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import TrashDialog, { type TrashedSession } from "./TrashDialog";
import { deferred } from "./testFixtures";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const item: TrashedSession = {
  key: "first",
  sessionId: "Session one",
  title: "核对会话恢复",
  sourcePath: "C:/fixture/rollout.jsonl",
  deletedAt: "2026-10-02T10:00:00Z",
  fileCount: 2,
  state: "deleted",
  purgeRevision: "a".repeat(64),
};
const other: TrashedSession = { ...item, key: "second", title: "保留配置", purgeRevision: "b".repeat(64) };
const uiText = (zh: string) => zh;
const props = { onClose: vi.fn(), onRestored: vi.fn(), uiText };
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(invoke).mockResolvedValue([item]);
});
afterEach(cleanup);

async function openConfirmation() {
  await screen.findByText(item.title!);
  fireEvent.click(screen.getByRole("button", { name: `彻底删除：${item.title}` }));
  return screen.findByRole("dialog", { name: "彻底删除会话？" });
}

describe("permanent session trash deletion", () => {
  it("previews readable names, focuses Cancel and sends no mutation when cancelled", async () => {
    render(<TrashDialog open {...props} />);
    const dialog = await openConfirmation();
    expect(dialog.textContent).toContain("核对会话恢复");
    expect(dialog.textContent).toContain("此操作无法撤销");
    await waitFor(() => expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "取消" })));
    fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "彻底删除会话？" })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: `彻底删除：${item.title}` })),
    );
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(props.onRestored).not.toHaveBeenCalled();
    expect(screen.getByText(item.title!)).toBeTruthy();
  });

  it("clears exactly the confirmed revisions and retains a later entry on refresh", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([item, other]);
    render(<TrashDialog open {...props} />);
    await screen.findByText(other.title!);
    fireEvent.click(screen.getByRole("button", { name: "清空最近删除" }));
    const dialog = await screen.findByRole("dialog", { name: "彻底删除会话？" });
    expect(dialog.textContent).toContain("这 2 条会话");
    expect(dialog.textContent).toContain(other.title!);
    const later = { ...item, key: "third", title: "稍后新增的记录" };
    vi.mocked(invoke)
      .mockResolvedValueOnce({ purged: [item.key, other.key], failed: [] })
      .mockResolvedValueOnce([later]);
    fireEvent.click(within(dialog).getByRole("button", { name: "彻底删除" }));
    await screen.findByText(later.title);
    expect(invoke).toHaveBeenNthCalledWith(2, "purge_session_trash", {
      targets: [
        { key: item.key, revision: item.purgeRevision },
        { key: other.key, revision: other.purgeRevision },
      ],
    });
    expect(screen.queryByText(item.title!)).toBeNull();
    expect(screen.queryByText(other.title!)).toBeNull();
    expect(screen.getByRole("status").textContent).toContain("已彻底删除 2 条会话");
    expect(props.onRestored).not.toHaveBeenCalled();
  });

  it("reports partial failure, refreshes its revision and requires another confirmation", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([item, other]);
    render(<TrashDialog open {...props} />);
    await screen.findByText(other.title!);
    fireEvent.click(screen.getByRole("button", { name: "清空最近删除" }));
    const dialog = await screen.findByRole("dialog", { name: "彻底删除会话？" });
    const refreshed = { ...other, purgeRevision: "c".repeat(64) };
    vi.mocked(invoke)
      .mockResolvedValueOnce({ purged: [item.key], failed: [{ key: other.key, reason: "removeFailed" }] })
      .mockResolvedValueOnce([refreshed]);
    fireEvent.click(within(dialog).getByRole("button", { name: "彻底删除" }));
    await screen.findByText("部分文件未能清理，请刷新后重试。");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: `彻底删除：${other.title}` }).hasAttribute("disabled")).toBe(false),
    );
    expect(screen.queryByText(item.title!)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: `彻底删除：${other.title}` }));
    const retry = await screen.findByRole("dialog", { name: "彻底删除会话？" });
    vi.mocked(invoke)
      .mockResolvedValueOnce({ purged: [other.key], failed: [] })
      .mockResolvedValueOnce([]);
    fireEvent.click(within(retry).getByRole("button", { name: "彻底删除" }));
    await screen.findByText("暂无可恢复的会话");
    expect(invoke).toHaveBeenNthCalledWith(4, "purge_session_trash", {
      targets: [{ key: other.key, revision: refreshed.purgeRevision }],
    });
  });

  it("blocks duplicate purge, restore and close while a mutation is pending", async () => {
    render(<TrashDialog open {...props} />);
    const dialog = await openConfirmation();
    const pending = deferred<unknown>();
    vi.mocked(invoke).mockReturnValueOnce(pending.promise).mockResolvedValueOnce([]);
    const confirm = within(dialog).getByRole("button", { name: "彻底删除" });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(within(dialog).getByRole("button", { name: "取消" }).hasAttribute("disabled")).toBe(true);
    expect(
      within(screen.getByText("最近删除").closest('[role="dialog"]') as HTMLElement)
        .getByText("恢复")
        .closest("button")!.disabled,
    ).toBe(true);
    expect(props.onClose).not.toHaveBeenCalled();
    await act(async () => {
      pending.resolve({ purged: [item.key], failed: [] });
      await pending.promise;
    });
    await screen.findByText("暂无可恢复的会话");
  });

  it("retains transport failures without leaking raw errors and offers list recovery", async () => {
    render(<TrashDialog open {...props} />);
    const dialog = await openConfirmation();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("masked-fixture-secret"));
    fireEvent.click(within(dialog).getByRole("button", { name: "彻底删除" }));
    await screen.findByText("清理未完成，请刷新列表后重试。");
    expect(document.body.textContent).not.toContain("masked-fixture-secret");
    expect(screen.getByText(item.title!)).toBeTruthy();
    expect(invoke).toHaveBeenCalledTimes(2);
    vi.mocked(invoke).mockResolvedValueOnce([item]);
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(3));
    expect(screen.queryByText("清理未完成，请刷新列表后重试。")).toBeNull();
  });

  it("keeps successful removals out of a stale list returned after reopening", async () => {
    const view = render(<TrashDialog open {...props} />);
    const dialog = await openConfirmation();
    const pending = deferred<unknown>();
    const stale = deferred<TrashedSession[]>();
    vi.mocked(invoke).mockReturnValueOnce(pending.promise).mockReturnValueOnce(stale.promise);
    fireEvent.click(within(dialog).getByRole("button", { name: "彻底删除" }));
    view.rerender(<TrashDialog open={false} {...props} />);
    await act(async () => {
      pending.resolve({ purged: [item.key], failed: [] });
      await pending.promise;
    });
    view.rerender(<TrashDialog open {...props} />);
    await act(async () => {
      stale.resolve([item]);
      await stale.promise;
    });
    await screen.findByText("暂无可恢复的会话");
    expect(screen.queryByText(item.title!)).toBeNull();
    expect(screen.queryByRole("dialog", { name: "彻底删除会话？" })).toBeNull();
  });

  it("shows unverified entries without enabling single or batch permanent deletion", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{ ...item, purgeRevision: null }]);
    render(<TrashDialog open {...props} />);
    await screen.findByText(item.title!);
    expect(screen.getByText("该记录需重新核对，暂时无法彻底删除。")).toBeTruthy();
    expect(screen.getByRole("button", { name: `彻底删除：${item.title}` }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "清空最近删除" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "恢复" }).hasAttribute("disabled")).toBe(false);
  });
  it("returns focus to Close when the deleted trigger disappears and list refresh is pending", async () => {
    render(<TrashDialog open {...props} />);
    const dialog = await openConfirmation();
    const refresh = deferred<TrashedSession[]>();
    vi.mocked(invoke)
      .mockResolvedValueOnce({ purged: [item.key], failed: [] })
      .mockReturnValueOnce(refresh.promise);
    fireEvent.click(within(dialog).getByRole("button", { name: "彻底删除" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "彻底删除会话？" })).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "关闭" })));
    await act(async () => {
      refresh.resolve([]);
      await refresh.promise;
    });
    await screen.findByText("暂无可恢复的会话");
  });
  it("keeps a failed cleanup visible even when a refreshed catalog no longer lists its entry", async () => {
    render(<TrashDialog open {...props} />);
    const dialog = await openConfirmation();
    vi.mocked(invoke)
      .mockResolvedValueOnce({ purged: [], failed: [{ key: item.key, reason: "removeFailed" }] })
      .mockResolvedValueOnce([]);
    fireEvent.click(within(dialog).getByRole("button", { name: "彻底删除" }));
    await screen.findByText("部分文件未能清理，请刷新后重试。");
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(3));
    expect(screen.getByText(item.title!)).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("部分文件未能清理");
    expect(screen.queryByText("暂无可恢复的会话")).toBeNull();
  });
});
