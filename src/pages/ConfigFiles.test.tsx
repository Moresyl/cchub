import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ConfigFiles from "./ConfigFiles";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../components/Toast";
import { setLocale, t } from "../lib/i18n";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../lib/appPreferences", () => ({ fetchVisibleApps: async () => ["codex"] }));
const { fixtureTree, treeOverrides } = vi.hoisted(() => ({
  treeOverrides: {} as Record<string, unknown>,
  fixtureTree: {
    name: "codex",
    path: "C:/fixture",
    is_dir: true,
    children: [
      { name: "config.toml", path: "C:/fixture/config.toml", is_dir: false, children: [] },
      { name: "auth.json", path: "C:/fixture/auth.json", is_dir: false, children: [] },
    ],
  },
}));
vi.mock("../hooks/queries", () => ({
  useConfigFiles: (rootId: string) => ({
    data: treeOverrides[rootId] ?? fixtureTree,
    isLoading: false,
    error: null,
    refetch: vi.fn(),
  }),
}));
vi.mock("../components/CodeEditor", () => ({
  default: ({ value, onChange }: { value: string; onChange: (value: string) => void }) => (
    <textarea aria-label="Raw configuration" value={value} onChange={(event) => onChange(event.target.value)} />
  ),
}));
vi.mock("../components/MarkdownEditor", () => ({ default: () => null }));
vi.mock("../components/OmoConfigSection", () => ({ default: () => null }));
vi.mock("../components/OpenClawConfigSection", () => ({ default: () => null }));
vi.mock("../components/HermesConfigSection", () => ({ default: () => null }));

const content =
  'model_provider = "custom"\nmodel = "old"\n[model_providers.custom]\nname = "Custom"\nbase_url = "https://fixture.test/v1"\nwire_api = "responses"\n';
const loaded = { content, apiKey: "old-key", fileRevision: "revision-one" };
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function saveButton() {
  return screen.getByRole("button", { name: t().common.save }) as HTMLButtonElement;
}
function raw() {
  return screen.getByRole("textbox", { name: "Raw configuration" }) as HTMLTextAreaElement;
}
function key() {
  return screen.getByLabelText("API Key") as HTMLInputElement;
}
async function open() {
  render(<ConfigFiles />);
  fireEvent.click(await screen.findByRole("button", { name: "config.toml" }));
  await waitFor(() => expect(key().value).toBe("old-key"));
  await screen.findByRole("textbox", { name: "Raw configuration" });
}
beforeEach(() => {
  setLocale("zh");
  vi.clearAllMocks();
  for (const rootId of Object.keys(treeOverrides)) delete treeOverrides[rootId];
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "get_config_roots") return [{ id: "codex", name: "Codex", path: "C:/fixture", exists: true }];
    if (command === "read_codex_toml_structured") return loaded;
    if (command === "read_config_file_content") return '{"auth":"other file"}';
    if (command === "write_codex_toml_structured")
      return { content: (args as { rawToml: string }).rawToml, fileRevision: "revision-two" };
    return null;
  });
});
afterEach(cleanup);

describe("configuration editor ownership", () => {
  it("opens and saves an external MCP file inside its tool tree without adding another tab", async () => {
    const path = "C:/external/custom.toml";
    treeOverrides.codex = {
      ...fixtureTree,
      children: [{ name: "MCP · custom.toml", path, is_dir: false, children: [] }],
    };
    const fallback = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((command, args) =>
      command === "read_config_file_content" ? Promise.resolve("original = true\n") : fallback(command, args),
    );
    render(<ConfigFiles />);
    expect(await screen.findByRole("button", { name: "Codex" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Codex MCP" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "MCP · custom.toml" }));
    await waitFor(() => expect(raw().value).toBe("original = true\n"));
    fireEvent.change(raw(), { target: { value: "updated = true\n" } });
    fireEvent.click(saveButton());
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("write_config_file_content", {
        path,
        content: "updated = true\n",
        expectedContent: "original = true\n",
      }),
    );
    expect(invoke).toHaveBeenCalledWith("read_config_file_content", { path });
    expect(invoke).not.toHaveBeenCalledWith("write_codex_toml_structured", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith(
      "write_config_file_content",
      expect.objectContaining({ path: "C:/fixture/config.toml" }),
    );
  });

  it("keeps Claude Desktop accessible and continues honoring managed tool visibility", async () => {
    const fallback = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((command, args) =>
      command === "get_config_roots"
        ? Promise.resolve([
            { id: "claude", name: "Claude", path: "C:/hidden", exists: true },
            { id: "mcode", name: "MiniMax Code", path: "C:/hidden-minimax", exists: true },
            { id: "claude-desktop", name: "Claude Desktop", path: "C:/desktop", exists: true },
          ])
        : fallback(command, args),
    );
    treeOverrides["claude-desktop"] = {
      name: "Desktop",
      path: "C:/desktop",
      is_dir: true,
      children: [
        {
          name: "claude_desktop_config.json",
          path: "C:/desktop/claude_desktop_config.json",
          is_dir: false,
          children: [],
        },
      ],
    };
    render(<ConfigFiles />);
    expect((await screen.findByRole("button", { name: "Claude Desktop" })).getAttribute("aria-pressed")).toBe("true");
    expect(screen.queryByRole("button", { name: "Claude" })).toBeNull();
    expect(screen.queryByRole("button", { name: "MiniMax Code" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "claude_desktop_config.json" }));
    await waitFor(() => expect(raw().value).toContain("other file"));
    expect(invoke).toHaveBeenCalledWith("read_config_file_content", { path: "C:/desktop/claude_desktop_config.json" });
  });

  it("saves the latest credential with the keyboard shortcut after several edits", async () => {
    await open();
    fireEvent.change(key(), { target: { value: "first-key" } });
    fireEvent.change(key(), { target: { value: "latest-key" } });
    fireEvent(window, new Event("cchub-shortcut-save"));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        "write_codex_toml_structured",
        expect.objectContaining({ apiKey: "latest-key" }),
      ),
    );
  });

  it("retains incomplete raw TOML, blocks saving and resumes the form after correction", async () => {
    await open();
    const invalid = content + 'broken = "unfinished';
    fireEvent.change(raw(), { target: { value: invalid } });
    expect(raw().value).toBe(invalid);
    expect(screen.getByRole("alert").textContent).toContain("TOML");
    expect(saveButton().disabled).toBe(true);
    fireEvent(window, new Event("cchub-shortcut-save"));
    expect(invoke).not.toHaveBeenCalledWith("write_codex_toml_structured", expect.anything());
    fireEvent.change(raw(), { target: { value: content + "# corrected\n" } });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(key().value).toBe("old-key");
    expect(saveButton().disabled).toBe(false);
  });

  it("keeps invalid numeric drafts visible without erasing TOML and retains them across other field edits", async () => {
    await open();
    const context = screen.getByLabelText("上下文窗口") as HTMLInputElement;
    fireEvent.change(context, { target: { value: "40k" } });
    expect(context.value).toBe("40k");
    expect(context.getAttribute("aria-invalid")).toBe("true");
    expect(raw().value).toBe(content);
    expect(saveButton().disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("模型 ID"), { target: { value: "new-model" } });
    expect(context.value).toBe("40k");
    expect(raw().value).toBe(content.replace('model = "old"', 'model = "new-model"'));
    fireEvent.change(context, { target: { value: "400,000" } });
    expect(context.value).toBe("400000");
    expect(context.getAttribute("aria-invalid")).toBe("false");
    expect(raw().value).toMatch(/"?model_context_window"? = 400000/);
    expect(saveButton().disabled).toBe(false);
  });

  it("counts pending numeric drafts as unsaved edits and discards them on revert or file change", async () => {
    await open();
    fireEvent.change(screen.getByLabelText("上下文窗口"), { target: { value: "invalid" } });
    fireEvent.click(screen.getByRole("button", { name: "auth.json" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: t().common.cancel }));
    fireEvent.click(screen.getByRole("button", { name: t().configFiles.revert }));
    expect((screen.getByLabelText("上下文窗口") as HTMLInputElement).value).toBe("");
    expect(saveButton().disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("上下文窗口"), { target: { value: "invalid-again" } });
    fireEvent.click(screen.getByRole("button", { name: "auth.json" }));
    fireEvent.click(await screen.findByRole("button", { name: "继续" }));
    await waitFor(() => expect(raw().value).toContain("other file"));
    fireEvent.click(screen.getByRole("button", { name: "config.toml" }));
    await waitFor(() => expect(key().value).toBe("old-key"));
    expect((screen.getByLabelText("上下文窗口") as HTMLInputElement).value).toBe("");
  });
  it("saves an API-key-only edit with the loaded file revision and uses the new revision next time", async () => {
    await open();
    expect(saveButton().disabled).toBe(true);
    fireEvent.change(key(), { target: { value: "new-key" } });
    expect(saveButton().disabled).toBe(false);
    fireEvent.click(saveButton());
    await waitFor(() => expect(saveButton().disabled).toBe(true));
    expect(invoke).toHaveBeenCalledWith(
      "write_codex_toml_structured",
      expect.objectContaining({
        expectedRevision: "revision-one",
        rawToml: content,
        apiKey: "new-key",
      }),
    );
    expect(invoke).not.toHaveBeenCalledWith("read_config_file_content", { path: "C:/fixture/config.toml" });
    fireEvent.change(key(), { target: { value: "next-key" } });
    fireEvent.click(saveButton());
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        "write_codex_toml_structured",
        expect.objectContaining({ expectedRevision: "revision-two" }),
      ),
    );
  });

  it("includes credential changes in discard confirmation and reverts both the raw draft and credential", async () => {
    await open();
    fireEvent.change(key(), { target: { value: "new-key" } });
    fireEvent.click(screen.getByRole("button", { name: "auth.json" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: t().common.cancel }));
    fireEvent.change(raw(), { target: { value: content + "# draft\n" } });
    fireEvent.click(screen.getByRole("button", { name: t().configFiles.revert }));
    expect(key().value).toBe("old-key");
    expect(raw().value).toBe(content);
    expect(saveButton().disabled).toBe(true);
  });

  it("retains newer text and credential edits when an earlier save finishes", async () => {
    const saved = deferred<{ content: string; fileRevision: string }>();
    await open();
    vi.mocked(invoke).mockImplementationOnce(() => saved.promise);
    fireEvent.change(key(), { target: { value: "submitted-key" } });
    fireEvent.click(saveButton());
    fireEvent.change(raw(), { target: { value: content + "# newer draft\n" } });
    fireEvent.change(key(), { target: { value: "newer-key" } });
    await act(async () =>
      saved.resolve({ content: content + "# normalized saved text\n", fileRevision: "revision-two" }),
    );
    expect(raw().value).toBe(content + "# newer draft\n");
    expect(key().value).toBe("newer-key");
    expect(saveButton().disabled).toBe(false);
    fireEvent.click(saveButton());
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        "write_codex_toml_structured",
        expect.objectContaining({
          expectedRevision: "revision-two",
          apiKey: "newer-key",
        }),
      ),
    );
  });

  it("keeps the draft on a failed or conflicting save", async () => {
    await open();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("Configuration changed externally"));
    fireEvent.change(key(), { target: { value: "draft-key" } });
    fireEvent.click(saveButton());
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("草稿已保留")));
    expect(key().value).toBe("draft-key");
    expect(raw().value).toBe(content);
    expect(saveButton().disabled).toBe(true);
    expect(screen.getByRole("button", { name: "重新加载配置" })).toBeTruthy();
  });

  it("ignores a save acknowledgement after the user confirms opening another file", async () => {
    const saved = deferred<{ content: string; fileRevision: string }>();
    await open();
    vi.mocked(invoke).mockImplementationOnce(() => saved.promise);
    fireEvent.change(key(), { target: { value: "submitted-key" } });
    fireEvent.click(saveButton());
    fireEvent.click(screen.getByRole("button", { name: "auth.json" }));
    fireEvent.click(await screen.findByRole("button", { name: "继续" }));
    await waitFor(() => expect(raw().value).toContain("other file"));
    await act(async () => saved.resolve({ content: "old save result", fileRevision: "revision-two" }));
    expect(raw().value).toContain("other file");
    expect(screen.queryByLabelText("API Key")).toBeNull();
    expect(saveButton().disabled).toBe(true);
    expect(showToast).not.toHaveBeenCalledWith("success", expect.anything());
  });

  it.each([false, true])("ignores an older file load after another file opens (failure=%s)", async (failure) => {
    const old = deferred<typeof loaded>();
    const fallback = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((command, args) =>
      command === "read_codex_toml_structured" ? old.promise : fallback(command, args),
    );
    render(<ConfigFiles />);
    fireEvent.click(await screen.findByRole("button", { name: "config.toml" }));
    fireEvent.click(screen.getByRole("button", { name: "auth.json" }));
    await waitFor(() => expect(raw().value).toContain("other file"));
    await act(async () => (failure ? old.reject(new Error("old read failure")) : old.resolve(loaded)));
    expect(raw().value).toContain("other file");
    expect(screen.queryByLabelText("API Key")).toBeNull();
    expect(showToast).not.toHaveBeenCalled();
  });

  it("does not offer to overwrite a file whose load failed", async () => {
    const fallback = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation((command, args) =>
      command === "read_codex_toml_structured"
        ? Promise.reject(new Error("Cannot read configuration"))
        : fallback(command, args),
    );
    render(<ConfigFiles />);
    fireEvent.click(await screen.findByRole("button", { name: "config.toml" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("Cannot read")));
    expect(saveButton().disabled).toBe(true);
    expect(screen.queryByLabelText("API Key")).toBeNull();
  });
});
