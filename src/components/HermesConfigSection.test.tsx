import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import HermesConfigSection from "./HermesConfigSection";
import { setLocale } from "../lib/i18n";
import { showToast } from "./Toast";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./Toast", () => ({ showToast: vi.fn() }));
vi.mock("./CodeEditor", () => ({
  default: ({ value, language, readOnly }: { value: string; language: string; readOnly: boolean }) => (
    <pre aria-label="Preview source" data-language={language} data-read-only={String(readOnly)}>
      {value}
    </pre>
  ),
}));

const original = JSON.stringify({
  config: { model: { provider: "fixture-provider", base_url: "https://fixture.test", default: "original-model" } },
  env: { FIXTURE_KEY: "private-fixture-secret" },
  metadata: { hermesApiKeyEnv: "FIXTURE_KEY" },
});

beforeEach(() => {
  vi.clearAllMocks();
  setLocale("zh");
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "read_tool_config") return original;
    if (command === "get_hermes_root_override") return "D:/fixture/hermes";
    return undefined;
  });
});

async function ready() {
  await waitFor(() => expect((screen.getByLabelText("默认模型") as HTMLInputElement).disabled).toBe(false));
}

function writes() {
  return vi.mocked(invoke).mock.calls.filter(([command]) => command === "write_tool_config");
}

describe("HermesConfigSection", () => {
  it("loads actual values into shared controls and keeps an unknown provider visible", async () => {
    render(<HermesConfigSection />);
    await ready();
    expect((screen.getByLabelText("默认模型") as HTMLInputElement).value).toBe("original-model");
    expect(screen.getByRole("combobox", { name: "供应商" }).textContent).toContain("fixture-provider");
    expect(screen.getByLabelText("API 地址").getAttribute("data-slot")).toBe("input");
    expect(screen.getByLabelText("API 密钥").getAttribute("type")).toBe("password");
    expect((screen.getByRole("button", { name: "保存配置" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByLabelText("Preview source")).toBeNull();
    expect(writes()).toHaveLength(0);
  });

  it("shows a collapsed JSON preview with a masked key and saves the actual key once", async () => {
    render(<HermesConfigSection />);
    await ready();
    fireEvent.click(screen.getByRole("button", { name: "配置预览" }));
    const preview = screen.getByLabelText("Preview source");
    expect(preview.getAttribute("data-language")).toBe("json");
    expect(preview.getAttribute("data-read-only")).toBe("true");
    expect(preview.textContent).toContain("••••••••");
    expect(preview.textContent).not.toContain("private-fixture-secret");
    fireEvent.change(screen.getByLabelText("默认模型"), { target: { value: "new-model" } });
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", "Hermes 配置已保存"));
    expect(writes()).toHaveLength(1);
    const payload = writes()[0][1] as { toolId: string; content: string };
    expect(payload.toolId).toBe("hermes");
    expect(JSON.parse(payload.content).env.FIXTURE_KEY).toBe("private-fixture-secret");
    expect(JSON.parse(payload.content).config.model.default).toBe("new-model");
    expect(JSON.parse(payload.content).metadata).toEqual({ hermesApiKeyEnv: "FIXTURE_KEY" });
    expect((screen.getByRole("button", { name: "保存配置" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("sends an explicit key removal when the user clears the configured API key", async () => {
    render(<HermesConfigSection />);
    await ready();
    fireEvent.change(screen.getByLabelText("API 密钥"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledTimes(1));
    expect(JSON.parse((writes()[0][1] as { content: string }).content).env).toEqual({ FIXTURE_KEY: "" });
  });

  it("renames only the selected credential variable and returns to an unchanged state", async () => {
    render(<HermesConfigSection />);
    await ready();
    fireEvent.change(screen.getByLabelText("密钥环境变量"), { target: { value: "RENAMED_KEY" } });
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledTimes(1));
    expect(JSON.parse((writes()[0][1] as { content: string }).content).env).toEqual({
      FIXTURE_KEY: "",
      RENAMED_KEY: "private-fixture-secret",
    });
    expect((screen.getByRole("button", { name: "保存配置" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it.each(["", "BAD-NAME"])("blocks a credential save with an invalid variable name", async (value) => {
    render(<HermesConfigSection />);
    await ready();
    fireEvent.change(screen.getByLabelText("密钥环境变量"), { target: { value } });
    const input = screen.getByLabelText("密钥环境变量");
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(document.getElementById(input.getAttribute("aria-describedby")!)?.textContent).toContain("有效");
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));
    expect(writes()).toHaveLength(0);
  });

  it("blocks concurrent save, reload and editing before the pending operation settles", async () => {
    let settle!: () => void;
    const held = new Promise<void>((resolve) => {
      settle = resolve;
    });
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "write_tool_config") return held;
      return command === "read_tool_config" ? original : null;
    });
    render(<HermesConfigSection />);
    await ready();
    fireEvent.change(screen.getByLabelText("默认模型"), { target: { value: "new-model" } });
    const save = screen.getByRole("button", { name: "保存配置" });
    const reload = screen.getByRole("button", { name: "重新读取" });
    act(() => {
      fireEvent.click(save);
      fireEvent.click(save);
      fireEvent.click(reload);
    });
    expect(writes()).toHaveLength(1);
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "read_tool_config")).toHaveLength(1);
    expect((screen.getByLabelText("默认模型") as HTMLInputElement).disabled).toBe(true);
    expect((reload as HTMLButtonElement).disabled).toBe(true);
    await act(async () => settle());
    await ready();
  });

  it("preserves a failed-save draft and supports an explicit retry", async () => {
    let fail = true;
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "write_tool_config" && fail) throw new Error("Fixture save failed");
      return command === "read_tool_config" ? original : null;
    });
    render(<HermesConfigSection />);
    await ready();
    fireEvent.change(screen.getByLabelText("默认模型"), { target: { value: "draft-model" } });
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));
    expect((await screen.findByRole("alert")).textContent).toContain("已保留输入内容");
    expect((screen.getByLabelText("默认模型") as HTMLInputElement).value).toBe("draft-model");
    expect(showToast).not.toHaveBeenCalled();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledTimes(1));
    expect(writes()).toHaveLength(2);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("retains the last draft after a failed reload but blocks saving until a successful read", async () => {
    render(<HermesConfigSection />);
    await ready();
    fireEvent.change(screen.getByLabelText("默认模型"), { target: { value: "unsaved-model" } });
    vi.mocked(invoke).mockRejectedValue(new Error("Fixture read failed"));
    fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
    expect((await screen.findByRole("alert")).textContent).toContain("读取失败");
    expect((screen.getByLabelText("默认模型") as HTMLInputElement).value).toBe("unsaved-model");
    expect((screen.getByRole("button", { name: "保存配置" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "保存配置" }));
    expect(writes()).toHaveLength(0);
    vi.mocked(invoke).mockImplementation(async (command) => (command === "read_tool_config" ? original : null));
    fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
    await ready();
    expect((screen.getByLabelText("默认模型") as HTMLInputElement).value).toBe("original-model");
  });

  it.each([
    "{private-fixture-secret",
    "[]",
    JSON.stringify({ config: {}, env: { FIXTURE_KEY: 1 } }),
    JSON.stringify({ config: { model: { default: 12 } }, env: {} }),
    JSON.stringify({ config: { model: null }, env: {} }),
    JSON.stringify({ config: {}, env: {}, metadata: { hermesApiKeyEnv: [] } }),
  ])("refuses malformed loaded data without inventing a usable form", async (content) => {
    vi.mocked(invoke).mockImplementation(async (command) => (command === "read_tool_config" ? content : null));
    render(<HermesConfigSection />);
    expect((await screen.findByRole("alert")).textContent).toContain("Invalid Hermes configuration snapshot");
    expect(screen.getByRole("alert").textContent).not.toContain("private-fixture-secret");
    expect((screen.getByRole("button", { name: "保存配置" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByRole("button", { name: "配置预览" })).toBeNull();
    expect(writes()).toHaveLength(0);
  });

  it("shows absent fields as empty and never guesses a key from an unrelated variable", async () => {
    vi.mocked(invoke).mockImplementation(async (command) =>
      command === "read_tool_config" ? '{"config":{},"env":{"UNRELATED":"value"}}' : null,
    );
    render(<HermesConfigSection />);
    await ready();
    for (const label of ["默认模型", "API 地址", "API 密钥", "密钥环境变量"])
      expect((screen.getByLabelText(label) as HTMLInputElement).value).toBe("");
    expect(screen.getByRole("combobox", { name: "供应商" }).textContent).toContain("未配置");
    expect(writes()).toHaveLength(0);
  });
});
