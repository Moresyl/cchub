import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppDialogProvider } from "./AppDialogProvider";
import WebDavSyncSection from "./WebDavSyncSection";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("./Toast", () => ({ showToast: vi.fn() }));

const settings = {
  enabled: false,
  base_url: "",
  username: "",
  password: "",
  has_password: false,
  remote_root: "cchub-sync",
  profile: "default",
  auto_sync: false,
  last_sync_at: null,
  last_error: null,
};

describe("WebDavSyncSection", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_webdav_sync_settings") return settings;
      if (command === "webdav_sync_fetch_remote_info") return null;
      return null;
    });
    listenMock.mockResolvedValue(vi.fn());
  });

  it("loads once and keeps one event subscription after state updates", async () => {
    const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });

    render(
      <QueryClientProvider client={client}>
        <AppDialogProvider>
          <WebDavSyncSection />
        </AppDialogProvider>
      </QueryClientProvider>,
    );

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_webdav_sync_settings"));
    await act(() => new Promise((resolve) => setTimeout(resolve, 50)));

    expect(invokeMock.mock.calls.filter(([command]) => command === "get_webdav_sync_settings")).toHaveLength(1);
    expect(listenMock).toHaveBeenCalledTimes(1);
  });
});
