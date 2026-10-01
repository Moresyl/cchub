import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ConfigFiles from "./ConfigFiles";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../components/Toast";
import { setLocale, t } from "../lib/i18n";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../lib/appPreferences", () => ({ fetchVisibleApps: async () => ["codex"] }));
const { fixtureTree } = vi.hoisted(() => ({
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
  useConfigFiles: () => ({
    data: fixtureTree,
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
}
beforeEach(() => {
  setLocale("zh");
  vi.clearAllMocks();
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
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("externally")));
    expect(key().value).toBe("draft-key");
    expect(raw().value).toBe(content);
    expect(saveButton().disabled).toBe(false);
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
