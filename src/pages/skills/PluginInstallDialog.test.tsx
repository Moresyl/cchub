import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import PluginInstallDialog from "./PluginInstallDialog";

describe("PluginInstallDialog", () => {
  it("requires a source and submits it from the keyboard", () => {
    const onConfirm = vi.fn();
    const setSource = vi.fn();
    const { rerender } = render(
      <PluginInstallDialog
        isOpen
        source=""
        setSource={setSource}
        busy={false}
        locale="zh"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "安装" }).hasAttribute("disabled")).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "插件归档地址" }), {
      target: { value: "https://example.com/plugin.zip" },
    });
    expect(setSource).toHaveBeenCalledWith("https://example.com/plugin.zip");

    rerender(
      <PluginInstallDialog
        isOpen
        source="https://example.com/plugin.zip"
        setSource={setSource}
        busy={false}
        locale="zh"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );
    fireEvent.keyDown(screen.getByRole("textbox", { name: "插件归档地址" }), { key: "Enter" });
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });
});
