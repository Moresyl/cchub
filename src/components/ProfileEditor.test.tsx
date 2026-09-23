import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProfileEditor from "./ProfileEditor";

function renderEditor(options: { saveDisabled?: boolean } = {}) {
  const onClose = vi.fn();
  const onSave = vi.fn();
  render(
    <ProfileEditor
      title="新增配置"
      subtitle="创建一个新的工具配置"
      onClose={onClose}
      onSave={onSave}
      saveDisabled={options.saveDisabled ?? false}
      saving={false}
    >
      <div>编辑内容</div>
    </ProfileEditor>,
  );
  return { onClose, onSave };
}

describe("ProfileEditor", () => {
  it("keeps header, content, and footer actions connected", () => {
    const { onClose, onSave } = renderEditor();

    expect(screen.getByRole("heading", { name: "新增配置" })).toBeTruthy();
    expect(screen.getByText("编辑内容")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "返回" }));
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));

    expect(onClose).toHaveBeenCalledTimes(2);
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it("disables save until the draft is valid", () => {
    const { onSave } = renderEditor({ saveDisabled: true });
    const saveButton = screen.getByRole("button", { name: "保存" });

    expect(saveButton.hasAttribute("disabled")).toBe(true);
    fireEvent.click(saveButton);
    expect(onSave).not.toHaveBeenCalled();
  });
});
