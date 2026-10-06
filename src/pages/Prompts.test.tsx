import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Prompts from "./Prompts";
import { deferred, record, snapshot } from "./prompts/testFixtures";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../lib/i18n", () => ({ getLocale: () => "zh" }));
vi.mock("../lib/appPreferences", () => ({ fetchVisibleApps: async () => ["claude", "codex"] }));
vi.mock("../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../components/CodeEditor", () => ({
  default: ({
    value,
    onChange,
    readOnly,
  }: {
    value: string;
    onChange?: (value: string) => void;
    readOnly?: boolean;
  }) => (
    <textarea
      aria-label="Instructions"
      value={value}
      readOnly={readOnly}
      onChange={(event) => onChange?.(event.target.value)}
    />
  ),
}));
vi.mock("../components/MarkdownPreview", () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "get_prompt_library_snapshot" ? snapshot() : null,
  );
});
afterEach(cleanup);
async function loaded() {
  render(<Prompts />);
  await screen.findByRole("heading", { name: "Fixture instructions" });
}
async function edit() {
  await loaded();
  fireEvent.click(screen.getByRole("button", { name: "编辑 Fixture instructions" }));
}
function calls(command: string) {
  return vi.mocked(invoke).mock.calls.filter(([name]) => name === command);
}

describe("prompt page interactions", () => {
  it("uses shared labelled search and tool controls and filters content", async () => {
    await loaded();
    expect(screen.getByRole("combobox", { name: "工具" }).getAttribute("data-slot")).toBe("select-trigger");
    const search = screen.getByRole("textbox", { name: "搜索 Prompt" });
    expect(search.getAttribute("data-slot")).toBe("input");
    fireEvent.change(search, { target: { value: "not matching" } });
    expect(screen.queryByRole("heading", { name: "Fixture instructions" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "清除搜索" }));
    expect(screen.getByRole("heading", { name: "Fixture instructions" })).toBeTruthy();
  });

  it("retains usable instructions when a stored timestamp is outside the supported date range", async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot({ prompts: { [record().id]: record({ updatedAt: 1e20 }) } }));
    await loaded();
    expect(screen.getByText("更新时间不可用")).toBeTruthy();
    expect(screen.getByRole("button", { name: "编辑 Fixture instructions" })).toBeTruthy();
  });

  it("keeps readable library data when the live file cannot be read and blocks activation", async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot({ live: null, liveError: "fixture permission error" }));
    await loaded();
    expect(screen.getByRole("alert").textContent).toContain("重新加载最新状态");
    expect(screen.getByRole("alert").textContent).not.toContain("fixture permission");
    expect(screen.getByRole("button", { name: "启用" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "新建" }));
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Draft" } });
    expect(screen.getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("button", { name: "保存并启用" }).hasAttribute("disabled")).toBe(true);
  });

  it("shows actual file mismatch instead of claiming the selected library version is active", async () => {
    vi.mocked(invoke).mockResolvedValue(snapshot({ live: { content: "external instructions", revision: "external" } }));
    await loaded();
    expect(screen.getByText("文件内容不同")).toBeTruthy();
    expect(screen.queryByText("当前启用")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "查看文件" }));
    expect(within(screen.getByRole("dialog")).getByRole("textbox").getAttribute("readonly")).not.toBeNull();
    expect((within(screen.getByRole("dialog")).getByRole("textbox") as HTMLTextAreaElement).value).toBe(
      "external instructions",
    );
  });

  it("asks before closing a dirty draft and can continue editing or discard", async () => {
    await edit();
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Changed" } });
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(screen.getByRole("dialog").textContent).toContain("放弃未保存");
    fireEvent.click(screen.getByRole("button", { name: "继续编辑" }));
    expect((screen.getByRole("textbox", { name: "名称" }) as HTMLInputElement).value).toBe("Changed");
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    fireEvent.click(screen.getByRole("button", { name: "放弃修改" }));
    expect(screen.queryByRole("textbox", { name: "名称" })).toBeNull();
    expect(calls("upsert_prompt")).toHaveLength(0);
  });

  it("keyboard save uses the latest draft and captured library/file revisions", async () => {
    await edit();
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Latest" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Instructions" }), { target: { value: "Newest text" } });
    await act(async () => {
      window.dispatchEvent(new Event("cchub-shortcut-save"));
    });
    expect(invoke).toHaveBeenCalledWith(
      "upsert_prompt",
      expect.objectContaining({
        app: "claude",
        expectedLibraryRevision: "library-1",
        expectedLiveRevision: "file-1",
        prompt: expect.objectContaining({ name: "Latest", content: "Newest text", enabled: true }),
      }),
    );
  });

  it("invalid drafts cannot bypass validation through keyboard save", async () => {
    await edit();
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "a".repeat(121) } });
    await act(async () => {
      window.dispatchEvent(new Event("cchub-shortcut-save"));
    });
    expect(calls("upsert_prompt")).toHaveLength(0);
  });

  it("marks an oversized stored description invalid and enables saving only after repair", async () => {
    vi.mocked(invoke).mockResolvedValue(
      snapshot({ prompts: { [record().id]: record({ description: "d".repeat(2001) }) } }),
    );
    await edit();
    const description = screen.getByRole("textbox", { name: "说明（可选）" });
    expect(description.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "保存并启用" }).hasAttribute("disabled")).toBe(true);
    await act(async () => {
      window.dispatchEvent(new Event("cchub-shortcut-save"));
    });
    expect(calls("upsert_prompt")).toHaveLength(0);
    fireEvent.change(description, { target: { value: "😀".repeat(2000) } });
    expect(description.getAttribute("aria-invalid")).toBeNull();
    expect((description as HTMLInputElement).maxLength).toBe(4000);
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(calls("upsert_prompt")).toHaveLength(1));
    expect(calls("upsert_prompt")[0][1]).toMatchObject({ prompt: { description: "😀".repeat(2000) } });
  });

  it("explains a file conflict without echoing internal failure details and retains the draft", async () => {
    await edit();
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Retained" } });
    vi.mocked(invoke).mockRejectedValueOnce("Prompt file changed externally; PRIVATE_VALUE");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("已被外部修改");
    expect(alert.textContent).not.toContain("PRIVATE_VALUE");
    expect((screen.getByRole("textbox", { name: "名称" }) as HTMLInputElement).value).toBe("Retained");
    expect(screen.getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(true);
  });

  it("preserves a rejected draft and retries only after explicit reload with new revisions", async () => {
    await edit();
    fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Retained draft" } });
    vi.mocked(invoke).mockRejectedValueOnce(new Error("external change"));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByRole("alert");
    expect((screen.getByRole("textbox", { name: "名称" }) as HTMLInputElement).value).toBe("Retained draft");
    expect(screen.getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(true);
    vi.mocked(invoke).mockResolvedValueOnce(
      snapshot({
        libraryRevision: "library-2",
        live: { content: "external", revision: "file-2" },
        prompts: { [record().id]: record({ content: "Externally updated stored instructions" }) },
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "重新加载" }));
    await screen.findByRole("status");
    fireEvent.click(screen.getByRole("button", { name: "查看库内版本" }));
    const currentVersion = within(screen.getByRole("dialog")).getByRole("textbox") as HTMLTextAreaElement;
    expect(currentVersion.value).toBe("Externally updated stored instructions");
    expect(currentVersion.readOnly).toBe(true);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect((screen.getByRole("textbox", { name: "名称" }) as HTMLInputElement).value).toBe("Retained draft");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(calls("upsert_prompt")).toHaveLength(2));
    expect(calls("upsert_prompt")[1][1]).toMatchObject({
      expectedLibraryRevision: "library-2",
      expectedLiveRevision: "file-2",
    });
  });

  it("locks all writes, draft fields and navigation while saving", async () => {
    await edit();
    const held = deferred<null>();
    vi.mocked(invoke).mockReturnValueOnce(held.promise);
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(screen.getByRole("textbox", { name: "名称" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("combobox").hasAttribute("disabled")).toBe(true);
    await act(async () => {
      window.dispatchEvent(new Event("cchub-shortcut-save"));
      window.dispatchEvent(new Event("cchub-shortcut-new"));
      window.dispatchEvent(new Event("cchub-shortcut-escape"));
    });
    expect(calls("upsert_prompt")).toHaveLength(1);
    expect(screen.queryByRole("dialog")).toBeNull();
    await act(async () => {
      held.resolve(null);
      await held.promise;
    });
    await waitFor(() => expect(screen.queryByRole("textbox", { name: "名称" })).toBeNull());
  });

  it("keeps a saved draft visible when readback fails and prevents accidental retry", async () => {
    await edit();
    vi.mocked(invoke).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error("readback failed"));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByRole("alert");
    expect(screen.getByRole("textbox", { name: "名称" })).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("修改已保存");
    expect(screen.getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(true);
    expect(calls("upsert_prompt")).toHaveLength(1);
  });

  it("confirmed deletion preserves the live-file policy and does not resurrect removed rows", async () => {
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "删除 Fixture instructions" }));
    expect(screen.getByRole("dialog").textContent).toContain("当前文件会保留");
    vi.mocked(invoke)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(snapshot({ prompts: {}, libraryRevision: "deleted" }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "删除" }));
    await screen.findByText("暂无 Prompt");
    expect(calls("delete_prompt")[0][1]).toMatchObject({
      app: "claude",
      id: record().id,
      expectedLibraryRevision: "library-1",
    });
  });
});
