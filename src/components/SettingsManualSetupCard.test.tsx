import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import SettingsManualSetupCard, { type SettingsManualSetupCardReport } from "./SettingsManualSetupCard";

afterEach(cleanup);
const report: SettingsManualSetupCardReport = {
  tool_id: "codex",
  tool_name: "Codex CLI",
  cli_available: false,
  cli_command: "codex",
  config_path: "config.toml",
  config_exists: false,
  mcp_config_path: "config.toml",
  mcp_config_exists: false,
  skills_dir: "skills",
  skills_dir_exists: false,
  config_dir: "config",
  config_dir_exists: false,
  has_custom_config_dir: false,
  has_custom_mcp_config_path: false,
  has_custom_skills_dir: false,
  manual_setup_kind: "codex_login",
  manual_setup_command: "codex login",
  manual_setup_path: "C:/用户/配置/auth.json",
};
function props() {
  return {
    report,
    description: "准备登录配置",
    installUrl: "https://example.test/setup",
    bootstrapping: false,
    copyCommandLabel: "复制登录命令",
    copyPathLabel: "复制路径",
    openPathLabel: "打开路径",
    prepareFileLabel: "准备配置",
    openDocsLabel: "查看文档",
    bootstrappingLabel: "准备中",
    commandToastLabel: "命令",
    pathToastLabel: "路径",
    openPathToastLabel: "打开",
    docsToastLabel: "文档",
    onCopy: vi.fn(),
    onOpen: vi.fn(),
    onBootstrap: vi.fn(),
  };
}
describe("manual setup actions", () => {
  it("dispatches exact command, path and tool identity without submitting its containing form", () => {
    const p = props();
    const submit = vi.fn();
    render(
      <form onSubmit={submit}>
        <SettingsManualSetupCard {...p} />
      </form>,
    );
    for (const name of ["复制登录命令", "复制路径", "打开路径", "准备配置", "查看文档"])
      fireEvent.click(screen.getByRole("button", { name }));
    expect(p.onCopy.mock.calls).toEqual([
      ["codex login", "命令"],
      [report.manual_setup_path, "路径"],
    ]);
    expect(p.onOpen.mock.calls).toEqual([
      [report.manual_setup_path, "打开"],
      [p.installUrl, "文档"],
    ]);
    expect(p.onBootstrap).toHaveBeenCalledExactlyOnceWith("codex", "Codex CLI");
    expect(submit).not.toHaveBeenCalled();
  });
  it("hides unavailable actions and prevents repeat preparation while busy", () => {
    const p = props();
    render(
      <SettingsManualSetupCard
        {...p}
        report={{ ...report, manual_setup_command: null, manual_setup_path: null }}
        installUrl={null}
        bootstrapping
      />,
    );
    expect(screen.getAllByRole("button")).toHaveLength(1);
    const button = screen.getByRole("button", { name: "准备中" });
    expect(button.hasAttribute("disabled")).toBe(true);
    expect(button.getAttribute("aria-busy")).toBe("true");
    fireEvent.click(button);
    expect(p.onBootstrap).not.toHaveBeenCalled();
  });
});
