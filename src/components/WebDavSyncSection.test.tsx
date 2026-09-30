import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
vi.mock("../lib/i18n", async (importOriginal) => ({ ...(await importOriginal<object>()), getLocale: () => "zh" }));

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

  function renderSection() {
    const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
    return render(
      <QueryClientProvider client={client}>
        <AppDialogProvider>
          <WebDavSyncSection />
        </AppDialogProvider>
      </QueryClientProvider>,
    );
  }

  it("blocks remote operations on an unsaved draft and removes stale password hints", async () => {
    const stored = { ...settings, enabled: true, base_url: "https://dav.test", username: "alice", has_password: true };
    invokeMock.mockImplementation(async (command: string, args?: { settings: typeof stored }) => {
      if (command === "get_webdav_sync_settings") return stored;
      if (command === "set_webdav_sync_settings") return { ...args?.settings, password: "", has_password: true };
      return null;
    });
    renderSection();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "上传当前快照" }).hasAttribute("disabled")).toBe(false),
    );
    const password = screen.getByLabelText("密码 / 应用密码") as HTMLInputElement;
    expect(password.placeholder).toContain("已保存");
    fireEvent.change(screen.getByLabelText("用户名"), { target: { value: "bob" } });
    expect(password.placeholder).toBe("");
    for (const label of ["刷新远端", "上传当前快照", "从远端恢复"]) {
      const button = screen.getByRole("button", { name: label });
      expect(button.hasAttribute("disabled")).toBe(true);
      fireEvent.click(button);
    }
    expect(screen.getByRole("status").textContent).toContain("未保存");
    expect(invokeMock.mock.calls.some(([command]) => command === "webdav_sync_upload")).toBe(false);
    fireEvent.change(password, { target: { value: "new-password" } });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "set_webdav_sync_settings",
        expect.objectContaining({
          passwordTouched: true,
          settings: expect.objectContaining({ username: "bob", password: "new-password" }),
        }),
      ),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "上传当前快照" }).hasAttribute("disabled")).toBe(false),
    );
    expect(password.value).toBe("");
  });

  it("preserves a draft when background sync reports completion", async () => {
    renderSection();
    await waitFor(() => expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(false));
    fireEvent.change(screen.getByLabelText("用户名"), { target: { value: "unsaved-user" } });
    const callback = listenMock.mock.calls[0][1];
    await act(async () => callback({ payload: { status: "success", message: "done", synced_at: null, error: null } }));
    expect((screen.getByLabelText("用户名") as HTMLInputElement).value).toBe("unsaved-user");
    expect(invokeMock.mock.calls.filter(([command]) => command === "get_webdav_sync_settings")).toHaveLength(1);
  });

  it("does not allow saving defaults after settings fail to load", async () => {
    invokeMock.mockRejectedValue(new Error("keyring unavailable"));
    renderSection();
    expect(await screen.findByRole("button", { name: "重新读取设置" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(true);
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
