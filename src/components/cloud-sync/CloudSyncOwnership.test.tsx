import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import WebDavSyncSection from "../WebDavSyncSection";
import S3SyncSection from "../S3SyncSection";
import { showToast } from "../Toast";

const { invokeMock, listenMock, confirmMock, language } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  confirmMock: vi.fn(),
  language: { value: "zh" },
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("../../lib/i18n", async (importOriginal) => ({
  ...(await importOriginal<object>()),
  getLocale: () => language.value,
}));
vi.mock("../Toast", () => ({ showToast: vi.fn() }));
vi.mock("../AppDialogProvider", () => ({ useAppDialog: () => ({ confirm: confirmMock }) }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

const dav = {
  enabled: true,
  base_url: "https://dav.test",
  username: "alice",
  password: "",
  has_password: true,
  remote_root: "cchub-sync",
  profile: "default",
  auto_sync: false,
  backup_encryption: { hasPassphrase: true },
  last_sync_at: null,
  last_error: null,
};
const s3 = {
  enabled: true,
  endpoint: "https://s3.test",
  region: "us-east-1",
  bucket: "backup",
  accessKeyId: "alice",
  secretAccessKey: "",
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
  compatible: true,
  encrypted: true,
  upload_review: { revision: "a".repeat(64), requiresConfirmation: false, conditionalSupported: true },
  uploadReview: { revision: "a".repeat(64), requiresConfirmation: false, conditionalSupported: true },
  profilePath: "old-bucket/default",
  remoteUrl: "https://s3.test/old-bucket",
  profile_path: "old/default",
};
const variants = [
  {
    name: "WebDAV",
    Component: WebDavSyncSection,
    read: "get_webdav_sync_settings",
    write: "set_webdav_sync_settings",
    fetch: "webdav_sync_fetch_remote_info",
    upload: "webdav_sync_upload",
    download: "webdav_sync_download",
    test: "webdav_test_connection",
    initial: dav,
    field: "username",
    input: "用户名",
    uploadButton: "上传当前快照",
  },
  {
    name: "S3",
    Component: S3SyncSection,
    read: "get_s3_sync_settings",
    write: "set_s3_sync_settings",
    fetch: "s3_sync_fetch_remote_info",
    upload: "s3_sync_upload",
    download: "s3_sync_download",
    test: "s3_test_connection",
    initial: s3,
    field: "accessKeyId",
    input: "Access Key ID",
    uploadButton: "上传快照",
  },
] as const;

beforeEach(() => {
  vi.clearAllMocks();
  language.value = "zh";
  invokeMock.mockReset();
  listenMock.mockReset();
  listenMock.mockResolvedValue(vi.fn());
  confirmMock.mockReset();
  confirmMock.mockResolvedValue(false);
  invokeMock.mockImplementation(async (command: string, args?: { settings?: unknown }) => {
    if (command === "get_webdav_sync_settings") return dav;
    if (command === "get_s3_sync_settings") return s3;
    if (command.startsWith("set_")) return args?.settings;
    if (command.endsWith("fetch_remote_info") || command.endsWith("sync_upload")) return remote;
    return "done";
  });
});

for (const v of variants) {
  describe(`${v.name} state ownership`, () => {
    function view(strict = false, onRestored?: () => Promise<void>) {
      const client = new QueryClient({ defaultOptions: { mutations: { retry: false }, queries: { retry: false } } });
      const content = (
        <QueryClientProvider client={client}>
          <v.Component onRestored={onRestored} />
        </QueryClientProvider>
      );
      return render(strict ? <StrictMode>{content}</StrictMode> : content);
    }
    async function ready() {
      await waitFor(() =>
        expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(false),
      );
    }
    async function readRemote() {
      fireEvent.click(screen.getByRole("button", { name: "刷新远端" }));
      await waitFor(() =>
        expect(screen.getByRole("button", { name: "从远端恢复" }).hasAttribute("disabled")).toBe(false),
      );
    }
    const calls = (command: string) => invokeMock.mock.calls.filter(([name]) => name === command).length;

    it("refreshes a clean visible window, coalesces focus events, and does not write to the vendor", async () => {
      view();
      await ready();
      const pending = deferred<typeof v.initial>();
      invokeMock.mockImplementation((command: string) =>
        command === v.read ? pending.promise : Promise.resolve(remote),
      );
      fireEvent(window, new Event("focus"));
      fireEvent(document, new Event("visibilitychange"));
      fireEvent(window, new Event("focus"));
      expect(calls(v.read)).toBe(2);
      await act(async () => pending.resolve({ ...v.initial, [v.field]: "changed-elsewhere" }));
      expect((screen.getByLabelText(v.input) as HTMLInputElement).value).toBe("changed-elsewhere");
      expect(calls(v.write)).toBe(0);
      expect(calls(v.upload)).toBe(0);
      expect(calls(v.download)).toBe(0);
    });

    it("preserves a dirty draft and its secret when window focus returns", async () => {
      view();
      await ready();
      fireEvent.change(screen.getByLabelText(v.input), { target: { value: "draft" } });
      fireEvent.change(screen.getByLabelText("备份密码"), { target: { value: "unsaved-secret" } });
      fireEvent(window, new Event("focus"));
      fireEvent(document, new Event("visibilitychange"));
      expect(calls(v.read)).toBe(1);
      expect((screen.getByLabelText(v.input) as HTMLInputElement).value).toBe("draft");
      expect((screen.getByLabelText("备份密码") as HTMLInputElement).value).toBe("unsaved-secret");
    });

    it("ignores focus while hidden and removes refresh listeners when closed", async () => {
      const mounted = view();
      await ready();
      const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
      try {
        fireEvent(window, new Event("focus"));
        fireEvent(document, new Event("visibilitychange"));
        expect(calls(v.read)).toBe(1);
        mounted.unmount();
        visibility.mockReturnValue("visible");
        fireEvent(window, new Event("focus"));
        fireEvent(document, new Event("visibilitychange"));
        expect(calls(v.read)).toBe(1);
      } finally {
        visibility.mockRestore();
      }
    });

    it("ignores a read started before editing even if the draft later returns to the saved value", async () => {
      view();
      await ready();
      const pending = deferred<typeof v.initial>();
      invokeMock.mockImplementation((command: string) =>
        command === v.read ? pending.promise : Promise.resolve(remote),
      );
      fireEvent(window, new Event("focus"));
      fireEvent.change(screen.getByLabelText(v.input), { target: { value: "temporary" } });
      fireEvent.change(screen.getByLabelText(v.input), { target: { value: "alice" } });
      await act(async () => pending.resolve({ ...v.initial, [v.field]: "late-old" }));
      expect((screen.getByLabelText(v.input) as HTMLInputElement).value).toBe("alice");
      expect(screen.queryByText(/有未保存的修改/)).toBeNull();
    });

    it("a saved account supersedes the old background read and late failure cannot release its busy state", async () => {
      view();
      await ready();
      const oldRead = deferred<typeof v.initial>();
      const save = deferred<typeof v.initial>();
      invokeMock.mockImplementation((command: string) => {
        if (command === v.read) return oldRead.promise;
        if (command === v.write) return save.promise;
        return Promise.resolve(remote);
      });
      fireEvent(window, new Event("focus"));
      fireEvent.change(screen.getByLabelText(v.input), { target: { value: "new-account" } });
      fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
      await act(async () => oldRead.reject(new Error("old account failed")));
      expect(screen.getByRole("button", { name: "保存中..." }).hasAttribute("disabled")).toBe(true);
      expect(vi.mocked(showToast).mock.calls.some(([type]) => type === "error")).toBe(false);
      await act(async () => save.resolve({ ...v.initial, [v.field]: "new-account" }));
      await ready();
      expect((screen.getByLabelText(v.input) as HTMLInputElement).value).toBe("new-account");
    });

    it("blocks refreshes while confirmation is pending and never restores after unmount", async () => {
      const mounted = view();
      await ready();
      await readRemote();
      const pending = deferred<boolean>();
      confirmMock.mockReturnValue(pending.promise);
      fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
      expect(screen.getByRole("button", { name: "等待确认" }).hasAttribute("disabled")).toBe(true);
      const reads = calls(v.read);
      fireEvent(window, new Event("focus"));
      expect(calls(v.read)).toBe(reads);
      mounted.unmount();
      await act(async () => pending.resolve(true));
      expect(calls(v.download)).toBe(0);
    });

    it("does not replace a completed save with an older successful read", async () => {
      view();
      await ready();
      const oldRead = deferred<typeof v.initial>();
      invokeMock.mockImplementation((command: string, args?: { settings?: unknown }) => {
        if (command === v.read) return oldRead.promise;
        if (command === v.write) return Promise.resolve(args?.settings);
        return Promise.resolve(remote);
      });
      fireEvent(window, new Event("focus"));
      fireEvent.change(screen.getByLabelText(v.input), { target: { value: "new-account" } });
      fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
      await ready();
      await act(async () => oldRead.resolve({ ...v.initial, [v.field]: "old-account" }));
      expect((screen.getByLabelText(v.input) as HTMLInputElement).value).toBe("new-account");
      expect(screen.queryByText(/有未保存的修改/)).toBeNull();
      expect(calls(v.write)).toBe(1);
    });

    it("unlocks after cancelling confirmation and allows exactly one subsequent restore", async () => {
      view();
      await ready();
      await readRemote();
      const pending = deferred<boolean>();
      confirmMock.mockReturnValueOnce(pending.promise).mockResolvedValue(true);
      fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
      fireEvent.click(screen.getByRole("button", { name: "等待确认" }));
      expect(confirmMock).toHaveBeenCalledTimes(1);
      await act(async () => pending.resolve(false));
      await ready();
      expect(calls(v.download)).toBe(0);
      fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
      await waitFor(() => expect(calls(v.download)).toBe(1));
      await ready();
      expect(confirmMock).toHaveBeenCalledTimes(2);
    });

    it("does not upload when review completes after the page is closed", async () => {
      const mounted = view();
      await ready();
      const pending = deferred<typeof remote>();
      invokeMock.mockImplementation((command: string) =>
        command === v.fetch ? pending.promise : Promise.resolve("done"),
      );
      fireEvent.click(screen.getByRole("button", { name: v.uploadButton }));
      mounted.unmount();
      await act(async () => pending.resolve(remote));
      expect(calls(v.upload)).toBe(0);
      expect(confirmMock).not.toHaveBeenCalled();
    });

    it("does not relabel a committed restore as failed when settings reread fails", async () => {
      view();
      await ready();
      await readRemote();
      confirmMock.mockResolvedValue(true);
      invokeMock.mockImplementation((command: string) =>
        command === v.read ? Promise.reject(new Error("read failed")) : Promise.resolve("done"),
      );
      fireEvent.click(screen.getByRole("button", { name: "从远端恢复" }));
      await waitFor(() => expect(showToast).toHaveBeenCalledWith("info", expect.stringContaining("快照已恢复")));
      expect(calls(v.download)).toBe(1);
      expect(vi.mocked(showToast).mock.calls.some(([type]) => type === "error")).toBe(false);
    });

    it("ignores reads and test errors after unmount and supports StrictMode initialization", async () => {
      const mounted = view(true);
      await ready();
      const pending = deferred<string>();
      invokeMock.mockImplementation((command: string) =>
        command === v.test ? pending.promise : Promise.resolve(v.initial),
      );
      fireEvent.click(screen.getByRole("button", { name: "测试连接" }));
      mounted.unmount();
      await act(async () => pending.reject(new Error("late connection failure")));
      expect(showToast).not.toHaveBeenCalled();
    });
  });
}

it("refreshes a clean WebDAV form on a background sync event without resubscribing", async () => {
  const client = new QueryClient();
  const mounted = render(
    <QueryClientProvider client={client}>
      <WebDavSyncSection />
    </QueryClientProvider>,
  );
  await waitFor(() => expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(false));
  const handler = listenMock.mock.calls.find(([name]) => name === "webdav-sync-status-updated")?.[1];
  expect(handler).toBeTypeOf("function");
  invokeMock.mockImplementation(async (command: string) =>
    command === "get_webdav_sync_settings" ? { ...dav, last_sync_at: "2026-10-01T00:00:00Z" } : remote,
  );
  await act(async () => handler({ payload: { status: "success" } }));
  expect(invokeMock.mock.calls.filter(([name]) => name === "get_webdav_sync_settings")).toHaveLength(2);
  expect(showToast).toHaveBeenCalledWith("success", "WebDAV 自动同步已完成");
  expect(listenMock).toHaveBeenCalledTimes(1);
  const dispose = await listenMock.mock.results[0].value;
  mounted.unmount();
  await act(async () => {});
  expect(dispose).toHaveBeenCalledOnce();
  vi.mocked(showToast).mockClear();
  const reads = invokeMock.mock.calls.filter(([name]) => name === "get_webdav_sync_settings").length;
  await act(async () => handler({ payload: { status: "success" } }));
  expect(showToast).not.toHaveBeenCalled();
  expect(invokeMock.mock.calls.filter(([name]) => name === "get_webdav_sync_settings")).toHaveLength(reads);
});

it("invalidates S3 remote metadata on a signing region change while keeping the custom-endpoint backup key", async () => {
  render(<S3SyncSection />);
  const restore = () => screen.getByRole("button", { name: "从远端恢复" });
  await waitFor(() => expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "刷新远端" }));
  await waitFor(() => expect(restore().hasAttribute("disabled")).toBe(false));
  await act(async () => fireEvent(window, new Event("focus")));
  expect(restore().hasAttribute("disabled")).toBe(false);
  invokeMock.mockImplementation(async (command: string) =>
    command === "get_s3_sync_settings" ? { ...s3, region: "eu-west-1" } : remote,
  );
  await act(async () => fireEvent(window, new Event("focus")));
  expect((screen.getByLabelText("区域") as HTMLInputElement).value).toBe("eu-west-1");
  expect(restore().hasAttribute("disabled")).toBe(true);
  expect((screen.getByLabelText("备份密码") as HTMLInputElement).placeholder).toBe("已保存，留空保持不变");
  expect(screen.getByRole("button", { name: "上传快照" }).hasAttribute("disabled")).toBe(false);
  expect(invokeMock.mock.calls.filter(([name]) => name === "s3_sync_fetch_remote_info")).toHaveLength(1);
});

it("switching S3 locale does not reread or clear an unsaved draft", async () => {
  const view = render(<S3SyncSection />);
  await waitFor(() => expect(screen.getByRole("button", { name: "保存设置" }).hasAttribute("disabled")).toBe(false));
  fireEvent.change(screen.getByLabelText("Bucket"), { target: { value: "draft-bucket" } });
  fireEvent.change(screen.getByLabelText("备份密码"), { target: { value: "unsaved-password" } });
  language.value = "en";
  view.rerender(<S3SyncSection />);
  expect((screen.getByLabelText("Bucket") as HTMLInputElement).value).toBe("draft-bucket");
  expect((screen.getByLabelText("Backup password") as HTMLInputElement).value).toBe("unsaved-password");
  expect(invokeMock.mock.calls.filter(([name]) => name === "get_s3_sync_settings")).toHaveLength(1);
});
