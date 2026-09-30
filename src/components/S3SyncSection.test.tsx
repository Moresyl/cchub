import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import S3SyncSection from "./S3SyncSection";

const { invokeMock, confirmMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), confirmMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("../lib/i18n", () => ({ getLocale: () => "zh" }));
vi.mock("./Toast", () => ({ showToast: vi.fn() }));
vi.mock("./AppDialogProvider", () => ({ useAppDialog: () => ({ confirm: confirmMock }) }));

const stored = {
  enabled: true,
  endpoint: "https://s3.test",
  region: "us-east-1",
  bucket: "backup",
  accessKeyId: "alice",
  hasSecretAccessKey: true,
  remoteRoot: "cchub-sync",
  profile: "default",
  autoSync: false,
  backupEncryption: { hasPassphrase: true },
  lastSyncAt: null,
  lastError: null,
};
const remote = {
  exists: true,
  remoteUrl: "https://s3.test/backup",
  snapshotPath: "snapshot.sql",
  updatedAt: null,
  sizeBytes: 100,
  compatible: true,
  encrypted: true,
  profilePath: "default",
};

beforeEach(() => {
  vi.clearAllMocks();
  confirmMock.mockResolvedValue(false);
  invokeMock.mockImplementation(async (command: string, args?: { settings: typeof stored }) => {
    if (command === "get_s3_sync_settings") return stored;
    if (command === "set_s3_sync_settings") return { ...args?.settings, secretAccessKey: "", hasSecretAccessKey: true };
    if (command === "s3_sync_fetch_remote_info") return remote;
    return null;
  });
});

describe("S3 cloud sync", () => {
  it("keeps connection and remote status available but blocks encrypted restore without a password", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_s3_sync_settings") return { ...stored, backupEncryption: { hasPassphrase: false } };
      if (command === "s3_sync_fetch_remote_info") return remote;
      return null;
    });
    render(<S3SyncSection />);
    await waitFor(() => expect(screen.getByRole("button", { name: "刷新远端" }).hasAttribute("disabled")).toBe(false));
    expect(screen.getByRole("button", { name: "测试连接" }).hasAttribute("disabled")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "刷新远端" }));
    await screen.findByText("加密备份");
    expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "上传快照" }).hasAttribute("disabled")).toBe(true);
    expect(confirmMock).not.toHaveBeenCalled();
  });
  it("blocks remote actions until edits are saved, and does not imply another account has a saved secret", async () => {
    render(<S3SyncSection />);
    await waitFor(() => expect(screen.getByRole("button", { name: "上传快照" }).hasAttribute("disabled")).toBe(false));
    const secret = screen.getByLabelText("Secret Access Key") as HTMLInputElement;
    expect(secret.value).toBe("");
    expect(secret.placeholder).toContain("已保存");
    fireEvent.change(screen.getByLabelText("Access Key ID"), { target: { value: "bob" } });
    expect(secret.placeholder).toBe("");
    for (const name of ["刷新远端", "上传快照", "从远端恢复"])
      expect(screen.getByRole("button", { name }).hasAttribute("disabled")).toBe(true);
    fireEvent.change(secret, { target: { value: "new-secret" } });
    fireEvent.change(screen.getByLabelText("备份密码"), { target: { value: "new-backup-password" } });
    fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "set_s3_sync_settings",
        expect.objectContaining({
          secretTouched: true,
          settings: expect.objectContaining({
            accessKeyId: "bob",
            secretAccessKey: "new-secret",
            backupEncryption: expect.objectContaining({ passphrase: "new-backup-password", passphraseTouched: true }),
          }),
        }),
      ),
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "上传快照" }).hasAttribute("disabled")).toBe(false));
    expect(secret.value).toBe("");
    expect((screen.getByLabelText("备份密码") as HTMLInputElement).value).toBe("");
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("requires confirmation before overwriting local data and cancellation performs no restore", async () => {
    render(<S3SyncSection />);
    await waitFor(() => expect(screen.getByRole("button", { name: "刷新远端" }).hasAttribute("disabled")).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "刷新远端" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(false),
    );
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "从远端恢复" })));
    expect(confirmMock).toHaveBeenCalledWith(expect.objectContaining({ tone: "warning" }));
    expect(invokeMock.mock.calls.some(([command]) => command === "s3_sync_download")).toBe(false);
    confirmMock.mockResolvedValue(true);
    fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("s3_sync_download", { allowPlaintext: false }));
  });

  it("allows explicitly confirmed legacy restore without an encryption password", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_s3_sync_settings") return { ...stored, backupEncryption: undefined };
      if (command === "s3_sync_fetch_remote_info") return { ...remote, encrypted: false };
      return null;
    });
    render(<S3SyncSection />);
    await waitFor(() => expect(screen.getByRole("button", { name: "刷新远端" }).hasAttribute("disabled")).toBe(false));
    expect(screen.getByRole("button", { name: "上传快照" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("switch", { name: "每 15 分钟自动上传" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "刷新远端" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(false),
    );
    confirmMock.mockResolvedValue(true);
    fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("s3_sync_download", { allowPlaintext: true }));
    expect(confirmMock).toHaveBeenCalledWith(expect.objectContaining({ message: expect.stringContaining("未加密") }));
  });

  it("blocks encrypted restore without a password and clears stale backup hints on bucket change", async () => {
    render(<S3SyncSection />);
    const password = (await screen.findByLabelText("备份密码")) as HTMLInputElement;
    await waitFor(() => expect(password.placeholder).toContain("已保存"));
    fireEvent.change(screen.getByLabelText("Bucket"), { target: { value: "other" } });
    expect(password.placeholder).not.toContain("已保存");
    expect(screen.getByRole("switch", { name: "每 15 分钟自动上传" }).hasAttribute("disabled")).toBe(true);
    fireEvent.change(password, { target: { value: "new-backup-password" } });
    expect(screen.getByRole("switch", { name: "每 15 分钟自动上传" }).hasAttribute("disabled")).toBe(false);
    fireEvent.change(password, { target: { value: "" } });
    expect(screen.getByRole("switch", { name: "每 15 分钟自动上传" }).hasAttribute("disabled")).toBe(true);
  });

  it("shows a spinner only on the running action", async () => {
    render(<S3SyncSection />);
    await waitFor(() => expect(screen.getByRole("button", { name: "测试连接" }).hasAttribute("disabled")).toBe(false));
    invokeMock.mockImplementation(() => new Promise(() => {}));
    fireEvent.click(screen.getByRole("button", { name: "测试连接" }));
    expect(screen.getByRole("button", { name: "测试中..." }).getAttribute("aria-busy")).toBe("true");
    expect(screen.getByRole("button", { name: "上传快照" }).querySelector(".animate-spin")).toBeNull();
    expect(screen.getByRole("button", { name: "上传快照" }).getAttribute("aria-busy")).toBe("false");
  });

  it("offers a retry and blocks saving defaults when the initial read fails", async () => {
    invokeMock.mockRejectedValue(new Error("keyring unavailable"));
    render(<S3SyncSection />);
    expect(await screen.findByRole("button", { name: "重新读取设置" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(true);
  });
});
