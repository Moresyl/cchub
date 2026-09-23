import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProfilesLoadState from "./LoadState";

const localeText = (zh: string) => zh;

describe("ProfilesLoadState", () => {
  it("shows a loading state", () => {
    render(<ProfilesLoadState error={null} localeText={localeText} onRetry={vi.fn()} />);
    expect(screen.getByText("加载中...")).toBeTruthy();
  });

  it("hides implementation details and retries", () => {
    const onRetry = vi.fn();
    render(<ProfilesLoadState error="TypeError: internal path C:\\secret" localeText={localeText} onRetry={onRetry} />);
    expect(screen.queryByText(/TypeError|secret/)).toBeNull();
    expect(screen.getByText(/暂时无法读取本机配置/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});
