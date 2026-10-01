import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import ConfigFiles from "../ConfigFiles";
import { setLocale, t } from "../../lib/i18n";
import { showToast } from "../../components/Toast";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../../lib/appPreferences", () => ({ fetchVisibleApps: async () => ["claude"] }));
const { tree } = vi.hoisted(() => ({
  tree: {
    name: "claude",
    path: "C:/fixture",
    is_dir: true,
    children: [
      { name: "settings.local.json", path: "C:/fixture/settings.local.json", is_dir: false, children: [] },
      { name: "other.json", path: "C:/fixture/other.json", is_dir: false, children: [] },
    ],
  },
}));
vi.mock("../../hooks/queries", () => ({ useConfigFiles: () => ({ data: tree, refetch: vi.fn() }) }));
vi.mock("../../components/CodeEditor", () => ({
  default: ({ value, onChange }: { value: string; onChange: (value: string) => void }) => (
    <textarea aria-label="Raw configuration" value={value} onChange={(event) => onChange(event.target.value)} />
  ),
}));
vi.mock("../../components/MarkdownEditor", () => ({ default: () => null }));
vi.mock("../../components/OmoConfigSection", () => ({ default: () => null }));
vi.mock("../../components/OpenClawConfigSection", () => ({ default: () => null }));
vi.mock("../../components/HermesConfigSection", () => ({ default: () => null }));

const original = '{"env":{}}';
const toggles = {
  hideAttribution: false,
  enableTeammates: false,
  maxThinkingTokens: false,
  maxThinkingTokensValue: "64000",
  enableToolSearch: false,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function raw() {
  return screen.getByRole("textbox", { name: "Raw configuration" }) as HTMLTextAreaElement;
}
function save() {
  return screen.getByRole("button", { name: t().common.save }) as HTMLButtonElement;
}
async function open() {
  render(<ConfigFiles />);
  fireEvent.click(await screen.findByRole("button", { name: "settings.local.json" }));
  await waitFor(() => expect(raw().value).toBe(original));
}
beforeEach(() => {
  setLocale("zh");
  vi.clearAllMocks();
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "get_config_roots") return [{ id: "claude", name: "Claude", path: "C:/fixture", exists: true }];
    if (command === "read_claude_config_toggles") return toggles;
    if (command === "read_config_file_content")
      return (args as { path: string }).path.endsWith("other.json") ? '{"other":true}' : original;
    return null;
  });
});
afterEach(cleanup);

describe("Claude quick toggle write ownership", () => {
  it("serializes all toggles and raw saves, retaining text entered while the toggle is pending", async () => {
    await open();
    const written = deferred<typeof toggles>();
    vi.mocked(invoke).mockImplementationOnce(() => written.promise);
    fireEvent.click(screen.getByRole("checkbox", { name: "隐藏署名" }));
    expect(screen.getAllByRole("checkbox").every((checkbox) => (checkbox as HTMLButtonElement).disabled)).toBe(true);
    fireEvent.click(screen.getByRole("checkbox", { name: "启用团队协作" }));
    fireEvent.change(raw(), { target: { value: '{"newer":"draft"}' } });
    expect(save().disabled).toBe(true);
    fireEvent(window, new Event("cchub-shortcut-save"));
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "write_claude_config_toggle")).toHaveLength(
      1,
    );
    expect(invoke).not.toHaveBeenCalledWith("write_config_file_content", expect.anything());
    vi.mocked(invoke).mockImplementationOnce(async () => '{"env":{},"attribution":{"commit":""}}');
    await act(async () => written.resolve({ ...toggles, hideAttribution: true }));
    expect(raw().value).toBe('{"newer":"draft"}');
    expect(save().disabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: t().configFiles.revert }));
    expect(raw().value).toContain('"attribution"');
    expect((screen.getByRole("checkbox", { name: "隐藏署名" }) as HTMLButtonElement).getAttribute("aria-checked")).toBe(
      "true",
    );
  });

  it("requires a reload after a failed toggle and leaves the old configuration intact", async () => {
    await open();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("Cannot update configuration"));
    fireEvent.click(screen.getByRole("checkbox", { name: "隐藏署名" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("Cannot update")));
    expect(raw().value).toBe(original);
    expect((screen.getByRole("checkbox", { name: "启用团队协作" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "重新加载配置" }));
    await waitFor(() =>
      expect((screen.getByRole("checkbox", { name: "启用团队协作" }) as HTMLButtonElement).disabled).toBe(false),
    );
    fireEvent.change(raw(), { target: { value: '{"new":"draft"}' } });
    fireEvent.click(save());
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("write_config_file_content", {
        path: "C:/fixture/settings.local.json",
        content: '{"new":"draft"}',
      }),
    );
  });

  it("blocks toggles while a raw save is pending", async () => {
    await open();
    const saved = deferred<null>();
    vi.mocked(invoke).mockImplementationOnce(() => saved.promise);
    fireEvent.change(raw(), { target: { value: '{"saved":true}' } });
    fireEvent.click(save());
    fireEvent.click(screen.getByRole("button", { name: t().configFiles.revert }));
    expect(screen.getAllByRole("checkbox").every((checkbox) => (checkbox as HTMLButtonElement).disabled)).toBe(true);
    fireEvent.click(screen.getByRole("checkbox", { name: "启用团队协作" }));
    expect(invoke).not.toHaveBeenCalledWith("write_claude_config_toggle", expect.anything());
    await act(async () => saved.resolve(null));
  });

  it("ignores a late toggle acknowledgement after another file opens", async () => {
    await open();
    const written = deferred<typeof toggles>();
    vi.mocked(invoke).mockImplementationOnce(() => written.promise);
    fireEvent.click(screen.getByRole("checkbox", { name: "隐藏署名" }));
    fireEvent.click(screen.getByRole("button", { name: "other.json" }));
    await waitFor(() => expect(raw().value).toContain('"other"'));
    await act(async () => written.resolve({ ...toggles, hideAttribution: true }));
    expect(raw().value).toContain('"other"');
    expect(showToast).not.toHaveBeenCalledWith("success", expect.anything());
  });

  it("does not let a failed post-write read expose the old JSON as a safe baseline", async () => {
    await open();
    const written = deferred<typeof toggles>();
    vi.mocked(invoke).mockImplementationOnce(() => written.promise);
    fireEvent.click(screen.getByRole("checkbox", { name: "隐藏署名" }));
    fireEvent.change(raw(), { target: { value: '{"newer":"draft"}' } });
    vi.mocked(invoke).mockRejectedValueOnce(new Error("Post-write read failed"));
    await act(async () => written.resolve({ ...toggles, hideAttribution: true }));
    expect(raw().value).toBe('{"newer":"draft"}');
    expect(save().disabled).toBe(true);
    fireEvent(window, new Event("cchub-shortcut-save"));
    expect(invoke).not.toHaveBeenCalledWith("write_config_file_content", expect.anything());
    fireEvent.click(screen.getByRole("button", { name: "重新加载配置" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: t().common.cancel }));
    expect(raw().value).toBe('{"newer":"draft"}');
  });

  it("keeps the raw editor available when malformed settings prevent reading toggles", async () => {
    const fallback = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((command, args) =>
      command === "read_claude_config_toggles" ? Promise.reject(new Error("Malformed env")) : fallback(command, args),
    );
    await open();
    expect(screen.getAllByRole("checkbox").every((checkbox) => (checkbox as HTMLButtonElement).disabled)).toBe(true);
    fireEvent.change(raw(), { target: { value: '{"env":{"ENABLE_TOOL_SEARCH":"true"}}' } });
    vi.mocked(invoke).mockImplementation((command, args) =>
      command === "read_claude_config_toggles"
        ? Promise.resolve({ ...toggles, enableToolSearch: true })
        : fallback(command, args),
    );
    fireEvent.click(save());
    await waitFor(() =>
      expect(
        (screen.getByRole("checkbox", { name: "启用工具搜索" }) as HTMLButtonElement).getAttribute("aria-checked"),
      ).toBe("true"),
    );
    expect(save().disabled).toBe(true);
  });
});
