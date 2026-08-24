import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import WelcomeDialog from "./WelcomeDialog";

describe("WelcomeDialog", () => {
  it("selects locale and theme before completing setup", () => {
    const onSelectLocale = vi.fn();
    const onSelectTheme = vi.fn();
    const onFinish = vi.fn();
    render(
      <WelcomeDialog
        open
        locale="zh"
        theme="dark"
        installedToolCount={4}
        profileCount={2}
        onSelectLocale={onSelectLocale}
        onSelectTheme={onSelectTheme}
        onFinish={onFinish}
      />,
    );

    expect(screen.getByRole("dialog", { name: "欢迎使用 CCHub" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "English" }));
    fireEvent.click(screen.getByRole("button", { name: "浅色" }));
    fireEvent.click(screen.getByRole("button", { name: "开始使用" }));

    expect(onSelectLocale).toHaveBeenCalledWith("en");
    expect(onSelectTheme).toHaveBeenCalledWith("light");
    expect(onFinish).toHaveBeenCalledTimes(1);
  });
});
