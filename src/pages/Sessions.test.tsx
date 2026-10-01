import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { showToast } from "../components/Toast";
import { fetchSessionsPageData } from "../hooks/queries";
import type { SessionDetail } from "./sessions/helpers";
import { deferred, detailFixture, sessionFixture } from "./sessions/testFixtures";
import Sessions from "./Sessions";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../components/SessionUsageActions", () => ({ default: () => null }));
vi.mock("../lib/i18n", () => ({ getLocale: () => "zh" }));
vi.mock("../hooks/queries", () => ({
  fetchSessionsPageData: vi.fn(),
  fetchVisibleAppsQuery: async () => ["codex", "claude"],
  queryKeys: { visibleApps: ["visible-apps"], sessions: (tool: string | null) => ["sessions", tool ?? "all"] },
}));

const first = sessionFixture({ title: "First source" });
const other = sessionFixture({ title: "Other source", source_path: "C:/fixture/two.jsonl" });
const clients: QueryClient[] = [];
const writeText = vi.fn();
const originalScroll = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollIntoView");

function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } });
  clients.push(client);
  return render(
    <QueryClientProvider client={client}>
      <Sessions />
    </QueryClientProvider>,
  );
}
function row(title: string) {
  const element = screen.getByText(title).closest('[role="button"]');
  if (!(element instanceof HTMLElement)) throw new Error("Session row missing");
  return element;
}
async function loaded() {
  mount();
  await screen.findByText(first.title);
}
beforeEach(() => {
  vi.clearAllMocks();
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() });
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  writeText.mockResolvedValue(undefined);
  vi.mocked(fetchSessionsPageData).mockResolvedValue([first, other]);
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "get_session_detail")
      return detailFixture((args as { sourcePath: string }).sourcePath === other.source_path ? other : first);
    return null;
  });
});
afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  if (originalScroll) Object.defineProperty(HTMLElement.prototype, "scrollIntoView", originalScroll);
  else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
});

describe("session page interactions", () => {
  it("selects checkboxes without opening details and opens the row with Enter", async () => {
    await loaded();
    const item = row(first.title);
    const checkbox = within(item).getByRole("checkbox");
    fireEvent.click(checkbox);
    expect(checkbox.getAttribute("aria-checked")).toBe("true");
    expect(invoke).not.toHaveBeenCalled();
    fireEvent.keyDown(checkbox, { key: " " });
    expect(invoke).not.toHaveBeenCalled();
    fireEvent.keyDown(item, { key: "Enter" });
    await screen.findByText("Fixture question");
    expect(invoke).toHaveBeenCalledWith(
      "get_session_detail",
      expect.objectContaining({ sourcePath: first.source_path }),
    );
  });

  it("has a working close button during loading and ignores the late result", async () => {
    const pending = deferred<SessionDetail>();
    vi.mocked(invoke).mockReturnValue(pending.promise);
    await loaded();
    fireEvent.click(row(first.title));
    fireEvent.click(await screen.findByRole("button", { name: "关闭详情" }));
    await act(async () => {
      pending.resolve(detailFixture(first));
      await pending.promise;
    });
    expect(screen.queryByText("Fixture question")).toBeNull();
    expect(screen.queryByRole("button", { name: "关闭详情" })).toBeNull();
  });

  it("retries detail errors without losing the selected source and can close errors", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("detail read failure"));
    await loaded();
    fireEvent.click(row(other.title));
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await screen.findByText("Fixture question");
    expect(invoke).toHaveBeenLastCalledWith(
      "get_session_detail",
      expect.objectContaining({ sourcePath: other.source_path }),
    );
    fireEvent.click(screen.getByRole("button", { name: "关闭详情" }));
    vi.mocked(invoke).mockRejectedValueOnce(new Error("another error"));
    fireEvent.click(row(first.title));
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "关闭详情" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("preserves readable details after a failed refresh and retries inline", async () => {
    await loaded();
    fireEvent.click(row(first.title));
    await screen.findByText("Fixture question");
    vi.mocked(fetchSessionsPageData).mockRejectedValueOnce(new Error("scan failure"));
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("已保留当前会话");
    expect(screen.getByText("Fixture question")).toBeTruthy();
    fireEvent.click(within(alert).getByRole("button", { name: "重试" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("retries an initial failure instead of presenting an empty list", async () => {
    vi.mocked(fetchSessionsPageData).mockRejectedValueOnce(new Error("initial failure"));
    mount();
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("无法加载会话");
    fireEvent.click(within(alert).getByRole("button", { name: "重试" }));
    await screen.findByText(first.title);
  });

  it("searches IDs and details using the shared accessible inputs", async () => {
    await loaded();
    const search = screen.getByRole("textbox", { name: "搜索会话" });
    expect(search.getAttribute("data-control-size")).toBe("md");
    fireEvent.change(search, { target: { value: "shared-id" } });
    await waitFor(() => expect(screen.getAllByRole("checkbox")).toHaveLength(2));
    fireEvent.click(row(other.title));
    await screen.findByText("Fixture question");
    const detailSearch = screen.getByRole("textbox", { name: "会话内搜索" });
    expect(detailSearch.getAttribute("data-control-size")).toBe("md");
    fireEvent.change(detailSearch, { target: { value: "not present" } });
    await screen.findByText("没有匹配的记录");
    fireEvent(window, new Event("cchub-shortcut-escape"));
    expect((detailSearch as HTMLInputElement).value).toBe("");
    expect(screen.getByRole("button", { name: "关闭详情" })).toBeTruthy();
    fireEvent(window, new Event("cchub-shortcut-escape"));
    expect(screen.queryByRole("button", { name: "关闭详情" })).toBeNull();
  });

  it("uses a keyboard-operable app dropdown and requests its selected scope", async () => {
    await loaded();
    const trigger = screen.getByRole("combobox", { name: "筛选 App" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const option = await screen.findByRole("option", { name: "Claude" });
    fireEvent.click(option);
    await waitFor(() => expect(fetchSessionsPageData).toHaveBeenCalledWith("claude"));
    expect(trigger.getAttribute("data-state")).toBe("closed");
  });

  it("copies without opening a row and handles clipboard errors", async () => {
    await loaded();
    fireEvent.click(within(row(first.title)).getByRole("button", { name: "复制恢复命令" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("codex resume shared-id"));
    expect(invoke).not.toHaveBeenCalled();
    writeText.mockRejectedValueOnce(new Error("clipboard denied"));
    fireEvent.click(within(row(other.title)).getByRole("button", { name: "复制恢复命令" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "复制失败，请重试"));
  });

  it("deletes the confirmed source while keeping newly opened details", async () => {
    const deleted = deferred<null>();
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "delete_session") return deleted.promise;
      if (command === "get_session_detail") return detailFixture(other);
      return null;
    });
    await loaded();
    fireEvent.click(within(row(first.title)).getByRole("button", { name: "删除会话" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(row(other.title));
    await screen.findByText("Fixture question");
    vi.mocked(fetchSessionsPageData).mockRejectedValueOnce(new Error("post-delete scan failure"));
    await act(async () => {
      deleted.resolve(null);
      await deleted.promise;
    });
    await screen.findByRole("alert");
    expect(screen.queryByText(first.title)).toBeNull();
    expect(screen.getByText("Fixture question")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("delete_session", {
      toolId: "codex",
      sessionId: first.id,
      sourcePath: first.source_path,
      sourceBackend: "file",
    });
  });
});
