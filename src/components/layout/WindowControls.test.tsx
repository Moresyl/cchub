import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import WindowControls, { detectDesktopPlatform, runWindowAction } from "./WindowControls";

describe("WindowControls", () => {
  it("detects supported desktop platforms", () => {
    expect(detectDesktopPlatform("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe("windows");
    expect(detectDesktopPlatform("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe("macos");
    expect(detectDesktopPlatform("Mozilla/5.0 (X11; Linux x86_64)")).toBe("linux");
    expect(detectDesktopPlatform("test-runner")).toBe("web");
  });

  it("renders custom controls on Windows and leaves macOS controls native", () => {
    const { rerender } = render(<WindowControls platform="windows" />);
    expect(screen.getByRole("button", { name: "最小化" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "最大化" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "关闭" })).toBeTruthy();

    rerender(<WindowControls platform="macos" />);
    expect(screen.queryByRole("button", { name: "最小化" })).toBeNull();
  });

  it("ignores native actions in a regular browser", async () => {
    await expect(runWindowAction("close")).resolves.toBeUndefined();
  });
});
