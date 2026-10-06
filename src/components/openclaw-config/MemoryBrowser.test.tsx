import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { setLocale } from "../../lib/i18n";
import MemoryBrowser, { type MemoryEntry } from "./MemoryBrowser";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../MarkdownPreview", () => ({ default: ({ content }: { content: string }) => <p>{content}</p> }));
const entries: MemoryEntry[] = ["A", "B"].map((name) => ({
  path: `/fixture/${name}.md`,
  file_name: `${name}.md`,
  source: "global",
  project_name: null,
  modified_at: null,
  preview: `Entry ${name}`,
}));
const mock = vi.mocked(invoke);
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (value: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function select(name: string) {
  fireEvent.click(screen.getByRole("button", { name: new RegExp(`${name}\\.md`) }));
}
function search(value: string) {
  fireEvent.change(screen.getByLabelText("搜索记忆"), { target: { value } });
  fireEvent.click(screen.getByRole("button", { name: "搜索记忆记录" }));
}
async function open() {
  render(<MemoryBrowser />);
  await screen.findByText("A contents");
}
beforeEach(() => {
  setLocale("zh");
  mock.mockReset();
  mock.mockImplementation(async (command, args) =>
    command === "search_openclaw_daily_memory"
      ? entries
      : (args as { path: string }).path.includes("A.md")
        ? "A contents"
        : "B contents",
  );
});
afterEach(cleanup);
describe("memory browsing request ownership", () => {
  it("does not search on every keystroke and retains the current entry during refresh", async () => {
    await open();
    select("B");
    await screen.findByText("B contents");
    fireEvent.change(screen.getByLabelText("搜索记忆"), { target: { value: "typed" } });
    expect(mock.mock.calls.filter(([name]) => name === "search_openclaw_daily_memory")).toHaveLength(1);
    search("  keyword  ");
    await waitFor(() =>
      expect(mock).toHaveBeenCalledWith("search_openclaw_daily_memory", { query: "keyword", limit: 40 }),
    );
    expect(screen.getByText("B contents")).toBeTruthy();
    expect(mock.mock.calls.filter(([name]) => name === "read_openclaw_daily_memory_content")).toHaveLength(2);
  });
  it("ignores an older search response after a newer result finishes", async () => {
    await open();
    const old = deferred<MemoryEntry[]>();
    const fallback = mock.getMockImplementation()!;
    mock.mockImplementation((command, args) =>
      command === "search_openclaw_daily_memory" && (args as { query: string }).query === "old"
        ? old.promise
        : command === "search_openclaw_daily_memory"
          ? Promise.resolve([entries[1]])
          : fallback(command, args),
    );
    search("old");
    search("new");
    await screen.findByText("B contents");
    await act(async () => {
      old.resolve([entries[0]]);
      await old.promise;
    });
    expect(screen.queryByRole("button", { name: /A\.md/ })).toBeNull();
    expect(screen.getByText("B contents")).toBeTruthy();
  });
  it("does not replace a newer selected entry with late content or an old error", async () => {
    await open();
    const old = deferred<string>();
    const fallback = mock.getMockImplementation()!;
    mock.mockImplementation((command, args) =>
      command === "read_openclaw_daily_memory_content" && (args as { path: string }).path.includes("A.md")
        ? old.promise
        : fallback(command, args),
    );
    select("A");
    select("B");
    await screen.findByText("B contents");
    await act(async () => {
      old.reject(new Error("PRIVATE_DETAIL"));
      try {
        await old.promise;
      } catch {
        /* Expected stale failure. */
      }
    });
    expect(screen.getByText("B contents")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });
  it("preserves the prior list and preview after search failure and permits retry", async () => {
    await open();
    mock.mockRejectedValueOnce(new Error("PRIVATE_DETAIL"));
    search("failed");
    await screen.findByRole("alert");
    expect(screen.getByText("A contents")).toBeTruthy();
    expect(screen.getByRole("button", { name: /B\.md/ })).toBeTruthy();
    expect(screen.queryByText(/PRIVATE_DETAIL/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });
  it("provides a content retry and treats an empty successful entry as empty", async () => {
    await open();
    mock.mockRejectedValueOnce(new Error("PRIVATE_DETAIL"));
    select("B");
    await screen.findByRole("alert");
    mock.mockResolvedValueOnce("");
    fireEvent.click(screen.getByRole("button", { name: "重试读取" }));
    await screen.findByText("此记录为空");
    expect(screen.queryByRole("alert")).toBeNull();
  });
  it("keeps a selection made while a search is pending", async () => {
    await open();
    const pending = deferred<MemoryEntry[]>();
    mock.mockImplementationOnce(() => pending.promise);
    search("pending");
    select("B");
    await screen.findByText("B contents");
    await act(async () => {
      pending.resolve([entries[0]]);
      await pending.promise;
    });
    expect(screen.getByText("B contents")).toBeTruthy();
  });
  it("clears the preview when the latest search has no entries", async () => {
    await open();
    mock.mockResolvedValueOnce([]);
    search("empty");
    await screen.findByText("没有匹配的记忆记录");
    expect(screen.queryByText("A contents")).toBeNull();
  });
});
