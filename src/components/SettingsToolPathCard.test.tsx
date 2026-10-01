import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import SettingsToolPathCard from "./SettingsToolPathCard";

afterEach(cleanup);
const tool = {
  id: "codex",
  name: "Codex CLI",
  mcp_config_path: "C:/default/config.toml",
  skills_dir: "C:/default/skills",
  installed: false,
  install_command: "install codex",
};
const customPath = {
  tool_id: "codex",
  config_dir: "C:/custom",
  mcp_config_path: "C:/custom/config.toml",
  skills_dir: "C:/custom/skills",
};
function props() {
  return {
    tool,
    customPath,
    locale: "zh" as const,
    saved: false,
    onSaveMcpPath: vi.fn(async (_id: string, value: string, defaultValue: string) => value.trim() || defaultValue),
    onSaveSkillsDir: vi.fn(async (_id: string, value: string, defaultValue: string) => value.trim() || defaultValue),
    onPickMcpPath: vi.fn(async () => "C:/picked/config.toml"),
    onPickSkillsDir: vi.fn(async (): Promise<string | null> => "C:/picked/skills"),
    onCopyInstallCommand: vi.fn(),
  };
}
function field(name: string) {
  return screen.getByRole("textbox", { name }) as HTMLInputElement;
}
describe("tool path drafts and actions", () => {
  it("adopts pristine refreshed paths, preserves active edits and saves the exact field only", async () => {
    const p = props();
    const view = render(<SettingsToolPathCard {...p} />);
    fireEvent.change(field("MCP"), { target: { value: " C:/draft/config.toml " } });
    const refreshed = { ...customPath, mcp_config_path: "C:/external/config.toml", skills_dir: "C:/external/skills" };
    view.rerender(<SettingsToolPathCard {...p} customPath={refreshed} />);
    expect(field("MCP").value).toBe(" C:/draft/config.toml ");
    expect(field("Skills").value).toBe(refreshed.skills_dir);
    fireEvent.blur(field("MCP"));
    await waitFor(() => expect(field("MCP").value).toBe("C:/draft/config.toml"));
    expect(p.onSaveMcpPath).toHaveBeenCalledExactlyOnceWith(
      "codex",
      " C:/draft/config.toml ",
      tool.mcp_config_path,
      refreshed,
    );
    expect(p.onSaveSkillsDir).not.toHaveBeenCalled();
    fireEvent.blur(field("Skills"));
    expect(p.onSaveSkillsDir).not.toHaveBeenCalled();
  });
  it("uses a successful chosen path immediately, while cancellation retains the draft", async () => {
    const p = props();
    render(<SettingsToolPathCard {...p} />);
    fireEvent.change(field("MCP"), { target: { value: "unsaved" } });
    fireEvent.click(screen.getByRole("button", { name: "选择 MCP 配置文件" }));
    await waitFor(() => expect(field("MCP").value).toBe("C:/picked/config.toml"));
    expect(p.onSaveMcpPath).not.toHaveBeenCalled();
    p.onPickSkillsDir.mockResolvedValueOnce(null);
    fireEvent.change(field("Skills"), { target: { value: "retained skills" } });
    fireEvent.click(screen.getByRole("button", { name: "选择 Skills 文件夹" }));
    await waitFor(() => expect(field("Skills").disabled).toBe(false));
    expect(field("Skills").value).toBe("retained skills");
  });
  it("serializes card actions, catches failed saves and retains a retryable draft", async () => {
    const p = props();
    let reject!: (error: Error) => void;
    p.onSaveMcpPath.mockImplementationOnce(
      () =>
        new Promise((_resolve, fail) => {
          reject = fail;
        }),
    );
    render(<SettingsToolPathCard {...p} />);
    fireEvent.change(field("MCP"), { target: { value: "draft" } });
    fireEvent.blur(field("MCP"));
    expect(field("MCP").disabled).toBe(true);
    expect(field("Skills").disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "选择 MCP 配置文件" }));
    expect(p.onPickMcpPath).not.toHaveBeenCalled();
    await act(async () => reject(new Error("https://secret.test/?token=hidden")));
    expect(screen.getByRole("alert").textContent).toContain("修改已保留");
    expect(screen.getByRole("alert").textContent).not.toContain("secret");
    expect(field("MCP").value).toBe("draft");
    expect(field("MCP").disabled).toBe(false);
    fireEvent.blur(field("MCP"));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(p.onSaveMcpPath).toHaveBeenCalledTimes(2);
  });
  it("restores a cleared field to the canonical default and localizes copy/status actions", async () => {
    const p = props();
    const submit = vi.fn();
    render(
      <form onSubmit={submit}>
        <SettingsToolPathCard {...p} saved />
      </form>,
    );
    fireEvent.change(field("Skills"), { target: { value: "  " } });
    fireEvent.blur(field("Skills"));
    await waitFor(() => expect(field("Skills").value).toBe(tool.skills_dir));
    fireEvent.click(screen.getByRole("button", { name: "复制安装命令" }));
    await waitFor(() =>
      expect(p.onCopyInstallCommand).toHaveBeenCalledExactlyOnceWith(tool.install_command, tool.name),
    );
    expect(submit).not.toHaveBeenCalled();
    expect(screen.getByRole("status").textContent).toContain("已保存");
  });
});
