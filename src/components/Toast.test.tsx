import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { showToast, ToastContainer } from "./Toast";

vi.mock("../lib/i18n", () => ({ t: () => ({ common: { close: "关闭" } }) }));

describe("ToastContainer", () => {
  it("shows a dismissible success message in the anchored stack", () => {
    render(<ToastContainer />);

    act(() => showToast("success", "检查完成", 10000));

    const toast = screen.getByRole("status");
    expect(toast.textContent).toContain("检查完成");
    expect(toast.parentElement?.className).toBe("app-toast-stack");

    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("announces errors immediately", () => {
    render(<ToastContainer />);
    act(() => showToast("error", "无法连接", 10000));
    expect(screen.getByRole("alert").textContent).toContain("无法连接");
  });
});
