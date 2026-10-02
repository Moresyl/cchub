import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ComponentType } from "react";
import type { ProjectProfile } from "../hooks/useProjectProfiles";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
let Panel: ComponentType;
let Switcher: ComponentType;
let useProfiles: typeof import("../hooks/useProjectProfiles").useProjectProfiles;
const first: ProjectProfile = {
  id: "one",
  name: "工作档案",
  description: "现有说明",
  snapshot: { version: 1, workspaceId: "ws", configProfileIds: ["a", "b"] },
  updatedAt: "2026-10-02T01:00:00Z",
  lastAppliedAt: null,
  isActive: false,
};
const second: ProjectProfile = {
  ...first,
  id: "two",
  name: "备用档案",
  snapshot: { ...first.snapshot, configProfileIds: ["c"] },
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((yes) => {
    resolve = yes;
  });
  return { resolve, promise };
}

beforeEach(async () => {
  vi.resetModules();
  invoke.mockReset();
  invoke.mockResolvedValue([first, second]);
  const { setLocale } = await import("../lib/i18n");
  setLocale("zh");
  Panel = (await import("./ProjectProfilePanel")).default;
  Switcher = (await import("./ProjectProfileSwitcher")).default;
  useProfiles = (await import("../hooks/useProjectProfiles")).useProjectProfiles;
});
afterEach(cleanup);

async function show() {
  render(
    <>
      <Switcher />
      <Panel />
    </>,
  );
  await screen.findByRole("article", { name: first.name });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: `应用档案 ${first.name}` }).matches(":disabled")).toBe(false),
  );
}

function draft(name = "新项目", description = "保留\n多行说明") {
  fireEvent.click(screen.getByRole("button", { name: "保存当前" }));
  fireEvent.change(screen.getByRole("textbox", { name: "档案名称" }), { target: { value: name } });
  fireEvent.change(screen.getByRole("textbox", { name: "说明（可选）" }), { target: { value: description } });
}

describe("project profile panel and shared title-bar selection", () => {
  it("deduplicates initial reads and uses labeled shared controls with Unicode limits", async () => {
    await show();
    expect(invoke.mock.calls.filter(([command]) => command === "get_project_profiles")).toHaveLength(1);
    draft("😀".repeat(120));
    expect(screen.getByRole("textbox", { name: "档案名称" }).getAttribute("data-control-size")).toBe("md");
    expect(screen.getByRole("textbox", { name: "说明（可选）" }).getAttribute("data-slot")).toBe("textarea");
    expect(screen.getByRole("button", { name: "保存档案" }).matches(":disabled")).toBe(false);
    fireEvent.change(screen.getByRole("textbox", { name: "档案名称" }), { target: { value: "😀".repeat(121) } });
    expect(screen.getByRole("button", { name: "保存档案" }).matches(":disabled")).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "档案名称" }), { target: { value: "name\u0085" } });
    expect(screen.getByRole("textbox", { name: "档案名称" }).getAttribute("aria-invalid")).toBe("true");
  });

  it("keeps failed create drafts and private errors hidden, then permits a checked retry", async () => {
    await show();
    draft();
    invoke.mockRejectedValueOnce(new Error("private-key-sentinel"));
    fireEvent.click(screen.getByRole("button", { name: "保存档案" }));
    expect((await screen.findByRole("alert")).textContent).toContain("已输入的内容仍保留");
    expect((screen.getByRole("textbox", { name: "档案名称" }) as HTMLInputElement).value).toBe("新项目");
    expect(document.body.textContent).not.toContain("private-key-sentinel");
    expect(screen.getByRole("button", { name: "保存档案" }).matches(":disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "刷新列表" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.getByRole("button", { name: "保存档案" }).matches(":disabled")).toBe(false);
  });

  it("distinguishes committed creation from a failed refresh and retains its returned record", async () => {
    await show();
    draft();
    const created = { ...first, id: "created", name: "新项目", isActive: true };
    invoke.mockResolvedValueOnce(created).mockRejectedValueOnce(new Error("private-read-sentinel"));
    fireEvent.submit(screen.getByRole("textbox", { name: "档案名称" }).closest("form")!);
    expect((await screen.findByRole("alert")).textContent).toContain("操作已完成");
    expect(screen.getByRole("article", { name: "新项目" })).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "档案名称" })).toBeNull();
    expect(document.body.textContent).not.toContain("private-read-sentinel");
    expect(invoke.mock.calls.filter(([command]) => command === "create_project_profile")).toHaveLength(1);
    expect(invoke.mock.calls.filter(([command]) => command === "get_project_profiles")).toHaveLength(2);
  });

  it("serializes different profile writes before render and locks both surfaces until refresh finishes", async () => {
    function DoubleApply() {
      const { mutate } = useProfiles();
      return (
        <button
          onClick={() => {
            void mutate("apply", { id: "one" });
            void mutate("apply", { id: "two" });
          }}
        >
          Race
        </button>
      );
    }
    render(
      <>
        <Switcher />
        <Panel />
        <DoubleApply />
      </>,
    );
    await screen.findByRole("article", { name: first.name });
    const write = deferred<{ profile: ProjectProfile; appliedProfileIds: string[] }>();
    const read = deferred<ProjectProfile[]>();
    invoke.mockReturnValueOnce(write.promise).mockReturnValueOnce(read.promise);
    fireEvent.click(screen.getByRole("button", { name: "Race" }));
    expect(invoke.mock.calls.filter(([command]) => command === "apply_project_profile")).toHaveLength(1);
    expect(screen.getByRole("combobox").matches(":disabled")).toBe(true);
    expect(screen.getByRole("button", { name: `应用档案 ${second.name}` }).matches(":disabled")).toBe(true);
    await act(async () => write.resolve({ profile: { ...first, isActive: true }, appliedProfileIds: ["a", "b"] }));
    expect(screen.getByRole("combobox").matches(":disabled")).toBe(true);
    await act(async () => read.resolve([{ ...first, isActive: true }, second]));
    await waitFor(() => expect(screen.getByRole("combobox").matches(":disabled")).toBe(false));
    expect(screen.getByRole("combobox").textContent).toContain(first.name);
  });

  it("ignores a late old read after an external refresh and clears a stale active selection", async () => {
    const old = deferred<ProjectProfile[]>();
    invoke.mockReturnValueOnce(old.promise).mockResolvedValueOnce([{ ...second, isActive: true }]);
    render(
      <>
        <Switcher />
        <Panel />
      </>,
    );
    await act(async () => window.dispatchEvent(new Event("cchub-project-profile-refresh")));
    await screen.findByRole("article", { name: second.name });
    await act(async () => old.resolve([{ ...first, isActive: true }]));
    expect(screen.queryByRole("article", { name: first.name })).toBeNull();
    expect(screen.getByRole("combobox").textContent).toContain(second.name);
    invoke.mockResolvedValueOnce([second]);
    await act(async () => window.dispatchEvent(new Event("cchub-project-profile-refresh")));
    await waitFor(() => expect(screen.getByRole("combobox").textContent).toContain("未选择档案"));
  });

  it("requires confirmation to replace a snapshot and refreshes only once after success", async () => {
    await show();
    fireEvent.click(screen.getByRole("button", { name: `更新快照 ${first.name}` }));
    const dialog = await screen.findByRole("dialog");
    expect(invoke.mock.calls.filter(([command]) => command === "update_project_profile")).toHaveLength(0);
    invoke.mockResolvedValueOnce({ ...first, updatedAt: "2026-10-02T02:00:00Z" });
    fireEvent.click(within(dialog).getByRole("button", { name: "更新快照" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(invoke).toHaveBeenCalledWith("update_project_profile", {
      id: "one",
      name: first.name,
      description: first.description,
      resnapshot: true,
    });
    expect(invoke.mock.calls.filter(([command]) => command === "get_project_profiles")).toHaveLength(2);
  });

  it("closes a failed deletion dialog and keeps the original profile and retry controls", async () => {
    await show();
    fireEvent.click(screen.getByRole("button", { name: `删除档案 ${first.name}` }));
    const dialog = await screen.findByRole("dialog");
    invoke.mockRejectedValueOnce("private-delete-sentinel");
    fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));
    await screen.findByRole("alert");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("article", { name: first.name })).toBeTruthy();
    expect(screen.getByRole("button", { name: "刷新列表" }).matches(":disabled")).toBe(false);
    expect(document.body.textContent).not.toContain("private-delete-sentinel");
  });

  it("keeps returned apply state and both cached profiles after refresh failure", async () => {
    await show();
    invoke
      .mockResolvedValueOnce({ profile: { ...first, isActive: true }, appliedProfileIds: ["a", "b"] })
      .mockRejectedValueOnce("private-sentinel");
    fireEvent.click(screen.getByRole("button", { name: `应用档案 ${first.name}` }));
    await screen.findByRole("alert");
    expect(screen.getAllByRole("article")).toHaveLength(2);
    expect(screen.getByRole("combobox").textContent).toContain(first.name);
    expect(screen.getByRole("combobox").matches(":disabled")).toBe(true);
  });
});
