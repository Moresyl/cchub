import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppDialogProvider } from "./AppDialogProvider";
import WebDavSyncSection from "./WebDavSyncSection";
import { showToast } from "./Toast";

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
    vi.clearAllMocks();
    invokeMock.mockReset();
    listenMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_webdav_sync_settings") return settings;
      if (command === "webdav_sync_fetch_remote_info") return null;
      return null;
    });
    listenMock.mockResolvedValue(vi.fn());
  });

  function renderSection(onRestored?: () => Promise<void>) {
    const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
    return render(
      <QueryClientProvider client={client}>
        <AppDialogProvider>
          <WebDavSyncSection onRestored={onRestored} />
        </AppDialogProvider>
      </QueryClientProvider>,
    );
  }

  it("refreshes after successful restore and does not report refresh failures as a failed restore", async () => {
    const onRestored = vi.fn(async () => {
      throw new Error("refresh unavailable");
    });
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_webdav_sync_settings")
        return { ...settings, enabled: true, backup_encryption: { hasPassphrase: true } };
      if (command === "webdav_sync_fetch_remote_info") return { exists: true, compatible: true, encrypted: true };
      return "restored";
    });
    renderSection(onRestored);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
    expect(onRestored).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "继续恢复" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("info", expect.stringContaining("快照已恢复")));
    expect(onRestored).toHaveBeenCalledOnce();
    expect(invokeMock.mock.calls.filter(([command]) => command === "webdav_sync_download")).toHaveLength(1);
    expect(vi.mocked(showToast).mock.calls.some(([type]) => type === "error")).toBe(false);
  });

  it("does not refresh restored state after cancellation or a failed restore", async () => {
    const onRestored = vi.fn(async () => {});
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_webdav_sync_settings")
        return { ...settings, enabled: true, backup_encryption: { hasPassphrase: true } };
      if (command === "webdav_sync_fetch_remote_info") return { exists: true, compatible: true, encrypted: true };
      throw new Error("restore rejected");
    });
    renderSection(onRestored);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
    fireEvent.click(await screen.findByRole("button", { name: "取消" }));
    expect(onRestored).not.toHaveBeenCalled();
    expect(invokeMock.mock.calls.some(([command]) => command === "webdav_sync_download")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
    fireEvent.click(await screen.findByRole("button", { name: "继续恢复" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("restore rejected")));
    expect(onRestored).not.toHaveBeenCalled();
  });

  it("confirms a new remote revision and sends exactly that revision once", async () => {
    const remote = {
      exists: true,
      compatible: true,
      encrypted: true,
      upload_review: { revision: "a".repeat(64), requiresConfirmation: true, conditionalSupported: true },
    };
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_webdav_sync_settings")
        return { ...settings, enabled: true, backup_encryption: { hasPassphrase: true } };
      if (command === "webdav_sync_fetch_remote_info" || command === "webdav_sync_upload") return remote;
      return null;
    });
    renderSection();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "上传当前快照" }).hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(screen.getByRole("button", { name: "上传当前快照" }));
    expect(await screen.findByRole("button", { name: "替换备份" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "上传当前快照" }).hasAttribute("disabled")).toBe(false),
    );
    expect(invokeMock.mock.calls.some(([command]) => command === "webdav_sync_upload")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "上传当前快照" }));
    fireEvent.click(await screen.findByRole("button", { name: "替换备份" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("webdav_sync_upload", {
        reviewedRevision: remote.upload_review.revision,
      }),
    );
    expect(invokeMock.mock.calls.filter(([command]) => command === "webdav_sync_upload")).toHaveLength(1);
  });

  it("keeps connection and remote status available but blocks encrypted restore without a password", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_webdav_sync_settings") return { ...settings, enabled: true };
      if (command === "webdav_sync_fetch_remote_info") return { exists: true, compatible: true, encrypted: true };
      return null;
    });
    renderSection();
    await waitFor(() => expect(screen.getByRole("button", { name: "刷新远端" }).hasAttribute("disabled")).toBe(false));
    expect(screen.getByRole("button", { name: "测试连接" }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("button", { name: "上传当前快照" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(true);
  });

  it("blocks remote operations on an unsaved draft and removes stale password hints", async () => {
    const stored = {
      ...settings,
      enabled: true,
      base_url: "https://dav.test",
      username: "alice",
      has_password: true,
      backup_encryption: { hasPassphrase: true },
    };
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
    fireEvent.change(screen.getByLabelText("备份密码"), { target: { value: "new-backup-password" } });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "set_webdav_sync_settings",
        expect.objectContaining({
          passwordTouched: true,
          settings: expect.objectContaining({
            username: "bob",
            password: "new-password",
            backup_encryption: expect.objectContaining({ passphrase: "new-backup-password", passphraseTouched: true }),
          }),
        }),
      ),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "上传当前快照" }).hasAttribute("disabled")).toBe(false),
    );
    expect(password.value).toBe("");
    expect((screen.getByLabelText("备份密码") as HTMLInputElement).value).toBe("");
  });

  it("preserves a draft when background sync reports completion", async () => {
    renderSection();
    await waitFor(() => expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(false));
    fireEvent.change(screen.getByLabelText("用户名"), { target: { value: "unsaved-user" } });
    fireEvent.change(screen.getByLabelText("备份密码"), { target: { value: "unsaved-backup-password" } });
    const callback = listenMock.mock.calls[0][1];
    await act(async () => callback({ payload: { status: "success", message: "done", synced_at: null, error: null } }));
    expect((screen.getByLabelText("用户名") as HTMLInputElement).value).toBe("unsaved-user");
    expect((screen.getByLabelText("备份密码") as HTMLInputElement).value).toBe("unsaved-backup-password");
    expect(invokeMock.mock.calls.filter(([command]) => command === "get_webdav_sync_settings")).toHaveLength(1);
  });

  it("requires explicit legacy consent and permits cancellation without restoring", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_webdav_sync_settings")
        return { ...settings, enabled: true, base_url: "https://dav.test", username: "alice" };
      if (command === "webdav_sync_fetch_remote_info") return { exists: true, compatible: true, encrypted: false };
      return "restored";
    });
    renderSection();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(false),
    );
    expect(screen.getByRole("button", { name: "上传当前快照" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
    expect(await screen.findByText(/这份旧备份未加密/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(invokeMock.mock.calls.some(([command]) => command === "webdav_sync_download")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
    fireEvent.click(await screen.findByRole("button", { name: "继续恢复" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("webdav_sync_download", { allowPlaintext: true }));
  });

  it("requires a saved password for encrypted restore and scopes saved hints to the profile", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_webdav_sync_settings")
        return { ...settings, enabled: true, backup_encryption: { hasPassphrase: true } };
      if (command === "webdav_sync_fetch_remote_info") return { exists: true, compatible: true, encrypted: true };
      return "restored";
    });
    renderSection();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
    fireEvent.click(await screen.findByRole("button", { name: "继续恢复" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("webdav_sync_download", { allowPlaintext: false }));
    await waitFor(() => expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(false));
    const password = screen.getByLabelText("备份密码") as HTMLInputElement;
    expect(password.placeholder).toContain("已保存");
    fireEvent.change(screen.getByLabelText("Profile 名称"), { target: { value: "other" } });
    expect(password.placeholder).not.toContain("已保存");
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
