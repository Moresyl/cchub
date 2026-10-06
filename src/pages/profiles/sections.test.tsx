import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProfileBasicInfoSection } from "./sections";

describe("profile basic information", () => {
  it("associates name and tool labels and keeps a locked tool fixed", () => {
    const onNameChange = vi.fn();
    const onToolChange = vi.fn();
    render(
      <ProfileBasicInfoSection
        locale="zh"
        localeText={(zh) => zh}
        tools={[{ id: "opencode", name: "OpenCode", installed: true }]}
        draftTool="opencode"
        draftName="Local"
        isStructured={false}
        syncTargetsLocked={true}
        draftTargetTools={["opencode"]}
        structuredInstalledTools={[]}
        onToolChange={onToolChange}
        onNameChange={onNameChange}
        onToggleDraftTargetTool={vi.fn()}
      />,
    );
    const name = screen.getByRole("textbox", { name: "配置名称" });
    expect(screen.getByLabelText("配置名称")).toBe(name);
    fireEvent.change(name, { target: { value: "Renamed" } });
    expect(onNameChange).toHaveBeenCalledTimes(1);
    const tool = screen.getByLabelText("工具");
    expect(tool).toHaveProperty("disabled", true);
    fireEvent.click(tool);
    expect(onToolChange).not.toHaveBeenCalled();
  });
});
