import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import TrashDialog, { type TrashedSession } from "./TrashDialog";
import { deferred } from "./testFixtures";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const item: TrashedSession = {
  key: "key-one",
  sessionId: "Session one",
  sourcePath: "C:/fixture/rollout.jsonl",
  deletedAt: "2026-10-02T10:00:00Z",
  fileCount: 2,
  state: "deleted",
};
const uiText = (zh: string) => zh;
const onRestored = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockResolvedValue([item]);
});
afterEach(cleanup);

describe("recently deleted sessions", () => {
  it("shows the retained conversation title instead of an internal identifier", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([{ ...item, title: "核对配置切换与会话恢复" }]);
    render(<TrashDialog open onClose={vi.fn()} onRestored={onRestored} uiText={uiText} />);
    await screen.findByText("核对配置切换与会话恢复");
    expect(screen.queryByText(item.sessionId)).toBeNull();
  });
  it("restores the selected recovery entry and refreshes the live list", async () => {
    render(<TrashDialog open onClose={vi.fn()} onRestored={onRestored} uiText={uiText} />);
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText(item.sessionId);
    vi.mocked(invoke).mockResolvedValueOnce(null);
    fireEvent.click(within(dialog).getByRole("button", { name: "恢复" }));
    await waitFor(() => expect(onRestored).toHaveBeenCalledTimes(1));
    expect(invoke).toHaveBeenLastCalledWith("restore_session_trash", { key: item.key });
    expect(screen.queryByText(item.sessionId)).toBeNull();
  });
  it("keeps collision errors and offers retry without discarding recovery entries", async () => {
    render(<TrashDialog open onClose={vi.fn()} onRestored={onRestored} uiText={uiText} />);
    await screen.findByText(item.sessionId);
    vi.mocked(invoke).mockRejectedValueOnce(new Error("Existing file was preserved"));
    fireEvent.click(screen.getByRole("button", { name: "恢复" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("恢复位置已有不同内容，已保留现有文件。");
    expect(screen.getByText(item.sessionId)).toBeTruthy();
    expect(onRestored).not.toHaveBeenCalled();
    vi.mocked(invoke).mockResolvedValueOnce(null);
    fireEvent.click(screen.getByRole("button", { name: "恢复" }));
    await waitFor(() => expect(onRestored).toHaveBeenCalledTimes(1));
  });
  it("keeps a successful restore removed when an older list arrives after reopening", async () => {
    const props = { onClose: vi.fn(), onRestored, uiText };
    const mounted = render(<TrashDialog open {...props} />);
    await screen.findByText(item.sessionId);
    const restoreResult = deferred<unknown>();
    const staleList = deferred<TrashedSession[]>();
    vi.mocked(invoke).mockReturnValueOnce(restoreResult.promise).mockReturnValueOnce(staleList.promise);
    const button = screen.getByRole("button", { name: "恢复" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(invoke).toHaveBeenCalledTimes(2);
    mounted.rerender(<TrashDialog open={false} {...props} />);
    mounted.rerender(<TrashDialog open {...props} />);
    await act(async () => {
      restoreResult.resolve(null);
      await restoreResult.promise;
    });
    await waitFor(() => expect(onRestored).toHaveBeenCalledTimes(1));
    await act(async () => {
      staleList.resolve([item]);
      await staleList.promise;
    });
    await screen.findByText("暂无可恢复的会话");
    expect(screen.queryByText(item.sessionId)).toBeNull();
  });
  it("retries a list error and ignores a response from a closed dialog", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("Read failure"));
    const props = { onClose: vi.fn(), onRestored, uiText };
    const mounted = render(<TrashDialog open {...props} />);
    await screen.findByRole("alert");
    const pending = deferred<TrashedSession[]>();
    vi.mocked(invoke).mockReturnValueOnce(pending.promise);
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    mounted.rerender(<TrashDialog open={false} {...props} />);
    await act(async () => {
      pending.resolve([item]);
      await pending.promise;
    });
    vi.mocked(invoke).mockResolvedValueOnce([]);
    mounted.rerender(<TrashDialog open {...props} />);
    await screen.findByText("暂无可恢复的会话");
    expect(screen.queryByText(item.sessionId)).toBeNull();
  });
  it("masks unknown restore and list errors while keeping the entry retryable", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("masked-fixture-secret list failure"));
    render(<TrashDialog open onClose={vi.fn()} onRestored={onRestored} uiText={uiText} />);
    await screen.findByText("无法读取最近删除列表，请重试。");
    expect(document.body.textContent).not.toContain("masked-fixture-secret");
    vi.mocked(invoke).mockResolvedValueOnce([item]);
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await screen.findByText(item.sessionId);
    vi.mocked(invoke).mockRejectedValueOnce(new Error("masked-fixture-secret restore failure"));
    fireEvent.click(screen.getByRole("button", { name: "恢复" }));
    await screen.findByText("恢复未完成，请检查配置目录后重试。");
    expect(document.body.textContent).not.toContain("masked-fixture-secret");
    expect(screen.getByText(item.sessionId)).toBeTruthy();
    expect(onRestored).not.toHaveBeenCalled();
  });
});
