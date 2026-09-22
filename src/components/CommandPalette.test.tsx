import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import CommandPalette from "./CommandPalette";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("CommandPalette", () => {
  it("shows the focused workspace and applies a profile", async () => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    );
    Element.prototype.scrollIntoView = vi.fn();
    const invokeMock = vi.mocked(invoke);
    invokeMock.mockImplementation(async (command) => {
      if (command === "get_config_profiles") return [{ id: "p1", name: "Primary API", tool_id: "claude" }];
      if (command === "apply_config_profile")
        return { toolId: "claude", profileId: "p1", activeProfileIds: ["p1"], appliedAt: "now" };
      return undefined;
    });
    const refresh = vi.fn();
    window.addEventListener("cchub-profiles-refresh", refresh);
    const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });

    render(
      <QueryClientProvider client={client}>
        <CommandPalette open onOpenChange={vi.fn()} navigate={vi.fn()} currentPath="/" />
      </QueryClientProvider>,
    );

    expect(screen.getByText("配置切换")).toBeTruthy();
    expect(screen.queryByText("Autopilot")).toBeNull();
    fireEvent.click(await screen.findByText("Primary API"));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("apply_config_profile", { id: "p1" }));
    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    window.removeEventListener("cchub-profiles-refresh", refresh);
  });
});
