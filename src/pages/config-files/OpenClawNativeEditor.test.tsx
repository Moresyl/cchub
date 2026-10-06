import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import ConfigFiles from "../ConfigFiles";
import { setLocale, t } from "../../lib/i18n";
import { at, hasOwn, isOpenClawConfigFile, object, patchDocument } from "./useOpenClawEditor";
import { showToast } from "../../components/Toast";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../components/Toast", () => ({ showToast: vi.fn() }));
vi.mock("../../lib/appPreferences", () => ({ fetchVisibleApps: async () => ["openclaw", "codex"] }));
vi.mock("../../hooks/queries", () => {
  const tree = {
    name: "native",
    path: "C:/fixture",
    is_dir: true,
    children: [
      { name: "custom.json", path: "C:/fixture/custom.json", is_dir: false, children: [] },
      { name: "other.json", path: "C:/fixture/other.json", is_dir: false, children: [] },
    ],
  };
  return {
    useConfigFiles: () => ({
      data: tree,
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    }),
  };
});
vi.mock("../../components/DeferredCodeEditor", () => ({
  default: ({
    value,
    onChange,
    readOnly,
    ariaLabel,
  }: {
    value: string;
    onChange?: (value: string) => void;
    readOnly?: boolean;
    ariaLabel?: string;
  }) => (
    <textarea
      aria-label={ariaLabel || "Raw configuration"}
      value={value}
      readOnly={readOnly}
      onChange={(event) => onChange?.(event.target.value)}
    />
  ),
}));
vi.mock("../../components/MarkdownPreview", () => ({
  default: ({ content }: { content: string }) => <p>{content}</p>,
}));
vi.mock("../../components/ui/simple-select", () => ({
  SimpleSelect: ({
    value,
    options,
    onValueChange,
    ariaLabel,
    id,
  }: {
    value: string;
    options: { value: string; label: string }[];
    onValueChange: (value: string) => void;
    ariaLabel?: string;
    id?: string;
  }) => (
    <select id={id} aria-label={ariaLabel} value={value} onChange={(event) => onValueChange(event.target.value)}>
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

const original = JSON.stringify({
  models: {
    mode: "merge",
    providers: {
      local: {
        baseUrl: "https://fixture.test/v1",
        apiKey: { source: "env", id: "KEY" },
        api: "custom-protocol",
        headers: { KEEP: "value" },
        models: [
          {
            id: "main",
            name: "Main",
            contextWindow: 1000,
            cost: { input: 1, output: 2, cacheRead: 3 },
            reasoning: true,
            extra: { enabled: true },
          },
          { id: "backup", contextWindow: 2000, cost: { input: 4, output: 5 }, extra: "retained" },
        ],
      },
      other: { baseUrl: "https://other.test", models: [{ id: "untouched" }] },
    },
  },
  env: { vars: { KEEP: "value" }, shellEnv: { enabled: true } },
  tools: { profile: "custom" },
  channels: { retained: true },
  agents: {
    defaults: {
      model: { primary: "local/main", fallbacks: ["local/backup"], extra: true },
      models: { "local/main": { alias: "Main alias", extra: true } },
      workspace: "unchanged",
    },
  },
});
const failure = new Set<string>();
const invokeMock = vi.mocked(invoke);
const memory = [
  {
    path: "/fixture/a.md",
    file_name: "A.md",
    source: "global",
    project_name: null,
    modified_at: null,
    preview: "Entry A",
  },
];
function calls(command: string) {
  return invokeMock.mock.calls.filter(([name]) => name === command);
}
function change(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}
function save() {
  return screen.getByRole("button", { name: t().common.save }) as HTMLButtonElement;
}
async function open() {
  render(<ConfigFiles />);
  fireEvent.click(await screen.findByRole("button", { name: "打开原生配置" }));
  await screen.findByLabelText("接口地址");
}
async function saved() {
  fireEvent.click(save());
  await waitFor(() => expect(calls("write_config_file_content").length).toBeGreaterThan(0));
  const writes = calls("write_config_file_content");
  return JSON.parse((writes[writes.length - 1][1] as { content: string }).content);
}
beforeEach(() => {
  setLocale("zh");
  failure.clear();
  vi.clearAllMocks();
  invokeMock.mockImplementation(async (command, args) => {
    if (failure.has(command)) throw new Error("PRIVATE_DETAIL");
    if (command === "get_config_roots")
      return [
        { id: "openclaw", name: "OpenClaw", path: "C:/fixture", exists: true, config_file: "C:/fixture/custom.json" },
        { id: "codex", name: "Codex", path: "C:/other", exists: true },
      ];
    if (command === "read_config_file_content") return original;
    if (command === "parse_openclaw_config_content") return JSON.parse((args as { content: string }).content);
    if (command === "edit_openclaw_config_content") return JSON.stringify((args as { desired: unknown }).desired);
    if (command === "search_openclaw_daily_memory") return memory;
    if (command === "read_openclaw_daily_memory_content") return "Memory A content";
    return null;
  });
});
afterEach(cleanup);

describe("native OpenClaw file ownership", () => {
  it("writes the configured custom file and retains all native extensions and credential references", async () => {
    await open();
    change("接口地址", "https://new.test/v1");
    change("显示名", "Renamed");
    const written = await saved();
    const expected = JSON.parse(original);
    expected.models.providers.local.baseUrl = "https://new.test/v1";
    expected.models.providers.local.models[0].name = "Renamed";
    expect(written).toEqual(expected);
    expect(calls("write_config_file_content")[0][1]).toMatchObject({
      path: "C:/fixture/custom.json",
      expectedContent: original,
    });
    expect(calls("write_tool_config")).toHaveLength(0);
    expect(calls("read_tool_config")).toHaveLength(0);
  });
  it("keeps a structured draft while searching and opening memory", async () => {
    await open();
    change("显示名", "KEEP_DRAFT");
    fireEvent.click(screen.getByRole("button", { name: "记忆与日志" }));
    await screen.findByText("Memory A content");
    change("搜索记忆", "typed query");
    expect(calls("search_openclaw_daily_memory")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "搜索记忆记录" }));
    await waitFor(() => expect(calls("search_openclaw_daily_memory")).toHaveLength(2));
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect((screen.getByLabelText("显示名") as HTMLInputElement).value).toBe("KEEP_DRAFT");
    expect(calls("read_config_file_content")).toHaveLength(1);
    expect(save().disabled).toBe(false);
  });
  it("synchronizes structured changes to raw JSON without writing the file", async () => {
    await open();
    change("显示名", "Native change");
    fireEvent.click(screen.getByRole("tab", { name: "原始 JSON5" }));
    await waitFor(() =>
      expect((screen.getByLabelText("OpenClaw 原始配置") as HTMLTextAreaElement).value).toContain("Native change"),
    );
    expect(calls("write_config_file_content")).toHaveLength(0);
    await waitFor(() => expect(save().disabled).toBe(false));
    fireEvent.click(screen.getByRole("tab", { name: "结构化字段" }));
    expect(((await screen.findByLabelText("显示名")) as HTMLInputElement).value).toBe("Native change");
  });
  it("retains a rejected save and retries against the same reviewed baseline", async () => {
    await open();
    failure.add("write_config_file_content");
    change("模型别名", "Retry alias");
    fireEvent.click(save());
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.anything()));
    expect(showToast).not.toHaveBeenCalledWith("error", expect.stringContaining("PRIVATE_DETAIL"));
    expect(((await screen.findByLabelText("模型别名")) as HTMLInputElement).value).toBe("Retry alias");
    failure.delete("write_config_file_content");
    await waitFor(() => expect(save().disabled).toBe(false));
    await saved();
    await waitFor(() => expect(calls("write_config_file_content")).toHaveLength(2));
    expect(calls("write_config_file_content")[1][1]).toMatchObject({ expectedContent: original });
  });
  it("retains conflicting edits and requires review before another save", async () => {
    const fallback = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation((command, args) =>
      command === "write_config_file_content"
        ? Promise.reject(new Error("Configuration changed externally; reload and review it before saving."))
        : fallback(command, args),
    );
    await open();
    change("显示名", "Conflict draft");
    fireEvent.click(save());
    await screen.findByRole("button", { name: "重新加载配置" });
    expect(save().disabled).toBe(true);
    expect(((await screen.findByLabelText("显示名")) as HTMLInputElement).value).toBe("Conflict draft");
    fireEvent(window, new Event("cchub-shortcut-save"));
    expect(calls("write_config_file_content")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "重新加载配置" }));
    await screen.findByRole("dialog");
    expect(calls("read_config_file_content")).toHaveLength(1);
  });
  it("guards root and file switching before the draft is serialized", async () => {
    await open();
    change("显示名", "Pending");
    fireEvent.click(screen.getByRole("button", { name: "Codex" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("未保存的修改")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
    expect((screen.getByLabelText("显示名") as HTMLInputElement).value).toBe("Pending");
    fireEvent.click(screen.getByRole("button", { name: "other.json" }));
    await screen.findByRole("dialog");
    expect(calls("read_config_file_content")).toHaveLength(1);
  });
  it("validates numeric drafts and preserves untouched cost fields", async () => {
    await open();
    change("输入成本", "invalid");
    expect(save().disabled).toBe(true);
    change("输入成本", "0.25");
    change("上下文窗口", "100.5");
    expect(save().disabled).toBe(true);
    change("上下文窗口", "4000");
    const written = await saved();
    expect(written.models.providers.local.models[0].cost).toEqual({ input: 0.25, output: 2, cacheRead: 3 });
    expect(written.models.providers.local.models[0].contextWindow).toBe(4000);
  });
  it("reindexes numeric drafts when an earlier model is deleted", async () => {
    await open();
    change("选择模型", "1");
    change("输入成本", "0.75");
    change("选择模型", "0");
    fireEvent.click(screen.getByRole("button", { name: "删除模型" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "确认" }));
    const written = await saved();
    expect(written.models.providers.local.models).toHaveLength(1);
    expect(written.models.providers.local.models[0]).toMatchObject({
      id: "backup",
      cost: { input: 0.75, output: 5 },
      extra: "retained",
    });
  });
  it("removes whitespace-only numeric overrides without coercing them to zero", async () => {
    await open();
    change("上下文窗口", "   ");
    change("输入成本", "   ");
    const written = await saved();
    expect(written.models.providers.local.models[0].contextWindow).toBeUndefined();
    expect(written.models.providers.local.models[0].cost).toEqual({ output: 2, cacheRead: 3 });
  });
  it("retains malformed defaults and reports their type without crashing or writing a replacement", async () => {
    const source = JSON.parse(original);
    source.agents.defaults = null;
    const fallback = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation((command, args) =>
      command === "read_config_file_content" ? Promise.resolve(JSON.stringify(source)) : fallback(command, args),
    );
    await open();
    change("新回退模型", "other/untouched");
    fireEvent.click(screen.getByRole("button", { name: "添加回退" }));
    expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("草稿已保留"));
    expect((screen.getByLabelText("新回退模型") as HTMLInputElement).value).toBe("other/untouched");
    expect(save().disabled).toBe(true);
    expect(calls("write_config_file_content")).toHaveLength(0);
  });
  it("supports fallback changes and preserves the default model's extensions", async () => {
    await open();
    change("新回退模型", "other/untouched");
    fireEvent.click(screen.getByRole("button", { name: "添加回退" }));
    fireEvent.click(screen.getByRole("button", { name: "移除回退模型 local/backup" }));
    const written = await saved();
    expect(written.agents.defaults.model).toEqual({
      primary: "local/main",
      fallbacks: ["other/untouched"],
      extra: true,
    });
  });
  it("blocks failed parsing without writing defaults and allows repair in raw mode", async () => {
    failure.add("parse_openclaw_config_content");
    render(<ConfigFiles />);
    fireEvent.click(await screen.findByRole("button", { name: "打开原生配置" }));
    await screen.findByRole("alert");
    expect(save().disabled).toBe(true);
    expect(screen.queryByText(/PRIVATE_DETAIL/)).toBeNull();
    failure.delete("parse_openclaw_config_content");
    fireEvent.click(screen.getByRole("tab", { name: "原始 JSON5" }));
    change("OpenClaw 原始配置", original + " ");
    fireEvent.click(screen.getByRole("tab", { name: "结构化字段" }));
    await screen.findByLabelText("接口地址");
    expect(calls("write_config_file_content")).toHaveLength(0);
  });
  it("uses the latest draft for keyboard saves", async () => {
    await open();
    change("显示名", "First");
    change("显示名", "Latest");
    fireEvent(window, new Event("cchub-shortcut-save"));
    await waitFor(() => expect(calls("write_config_file_content")).toHaveLength(1));
    expect(
      JSON.parse((calls("write_config_file_content")[0][1] as { content: string }).content).models.providers.local
        .models[0].name,
    ).toBe("Latest");
  });
  it("does not write or replace another file after a late draft serialization", async () => {
    let resolve!: (value: string) => void;
    const pending = new Promise<string>((done) => {
      resolve = done;
    });
    const fallback = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation((command, args) =>
      command === "edit_openclaw_config_content" ? pending : fallback(command, args),
    );
    await open();
    change("显示名", "Late draft");
    fireEvent.click(save());
    await waitFor(() => expect(calls("edit_openclaw_config_content")).toHaveLength(1));
    fireEvent.click(screen.getByRole("button", { name: "other.json" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "继续" }));
    await waitFor(() =>
      expect((screen.getByLabelText("Raw configuration") as HTMLTextAreaElement).value).toBe(original),
    );
    await act(async () => {
      resolve('{"late":true}');
      await pending;
    });
    expect(calls("write_config_file_content")).toHaveLength(0);
    expect((screen.getByLabelText("Raw configuration") as HTMLTextAreaElement).value).toBe(original);
  });
});

describe("native document edits", () => {
  it("edits array descendants without mutating the baseline or losing siblings", () => {
    const root = JSON.parse(original);
    const next = patchDocument(root, ["models", "providers", "local", "models", "0", "cost", "input"], 0);
    expect(at(next, ["models", "providers", "local", "models", "0", "cost", "input"])).toBe(0);
    expect(root).toEqual(JSON.parse(original));
    expect(at(next, ["models", "providers", "local", "models", "1"])).toEqual(root.models.providers.local.models[1]);
  });
  it("supports exact prototype-like provider names without prototype mutation", () => {
    const next = patchDocument({}, ["models", "providers", "__proto__", "models"], []);
    expect(at(next, ["models", "providers", "__proto__", "models"])).toEqual([]);
    expect(at({}, ["__proto__"])).toBeUndefined();
    expect(({} as Record<string, unknown>).models).toBeUndefined();
    expect(hasOwn(next, "models")).toBe(true);
    expect(object([])).toBe(false);
  });
  it("rejects invalid containers and array indices without changing the input", () => {
    expect(() => patchDocument({ models: [] }, ["models", "-1", "id"], "bad")).toThrow();
    expect(() => patchDocument({ models: [] }, ["models", "0", "id"], "bad")).toThrow();
    expect(() => patchDocument({ models: [] }, ["models", "0"], "bad")).toThrow();
    expect(() => patchDocument({ models: null }, ["models", "providers"], {})).toThrow();
    expect(() => patchDocument({}, [], "bad")).toThrow();
    expect(patchDocument({ a: 1, retained: true }, ["a"], undefined)).toEqual({ retained: true });
  });
  it("binds native controls to the configured path rather than a filename guess", () => {
    expect(isOpenClawConfigFile("openclaw", "C:\\Fixture\\custom.json", "c:/fixture/custom.json")).toBe(true);
    expect(isOpenClawConfigFile("openclaw", "/fixture/A.json", "/fixture/a.json")).toBe(false);
    expect(isOpenClawConfigFile("codex", "C:/same.json", "C:/same.json")).toBe(false);
    expect(isOpenClawConfigFile("openclaw", null, undefined)).toBe(false);
  });
});
