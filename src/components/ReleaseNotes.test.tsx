import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ReleaseNotes from "./ReleaseNotes";
import { setLocale } from "../lib/i18n";
const { open, toast } = vi.hoisted(() => ({ open: vi.fn(), toast: vi.fn() }));
vi.mock("@tauri-apps/plugin-shell", () => ({ open }));
vi.mock("./Toast", () => ({ showToast: toast }));
beforeEach(() => {
  setLocale("zh");
  open.mockReset().mockResolvedValue(undefined);
  toast.mockReset();
});

describe("ReleaseNotes", () => {
  it("renders headings, lists and tables and opens links in the system browser", async () => {
    render(
      <ReleaseNotes
        content={
          "## Features\n\n- A **clear** update\n\n[Guide](https://example.test/guide)\n\n| File | OS |\n|---|---|\n| app.exe | Windows |"
        }
      />,
    );
    await act(() => vi.dynamicImportSettled());
    expect(await screen.findByRole("heading", { name: "Features" })).toBeTruthy();
    expect(screen.getByRole("list")).toBeTruthy();
    expect(screen.getByRole("table")).toBeTruthy();
    const link = screen.getByRole("link", { name: "Guide" });
    fireEvent.click(link);
    expect(open).toHaveBeenCalledExactlyOnceWith("https://example.test/guide");
  });
  it("does not execute raw HTML, navigate unsafe links or request remote images", async () => {
    const { container } = render(
      <ReleaseNotes
        content={
          "[Unsafe](javascript:alert%281%29) [Relative](/admin) ![Image description](https://example.test/image.png)\n\n<script>alert(1)</script>"
        }
      />,
    );
    await act(() => vi.dynamicImportSettled());
    expect(await screen.findByText("Unsafe")).toBeTruthy();
    expect(screen.queryByRole("link")).toBeNull();
    expect(screen.queryByRole("img")).toBeNull();
    expect(container.querySelector("script")).toBeNull();
    fireEvent.click(screen.getByText("Unsafe"));
    expect(open).not.toHaveBeenCalled();
  });
  it("reports an external-link failure without navigating the application", async () => {
    open.mockRejectedValueOnce(new Error("shell failed"));
    render(<ReleaseNotes content="[Guide](https://example.test)" />);
    await act(() => vi.dynamicImportSettled());
    fireEvent.click(await screen.findByRole("link", { name: "Guide" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("error", "无法打开链接，请稍后重试"));
  });
});
