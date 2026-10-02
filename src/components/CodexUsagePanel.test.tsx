import { act, fireEvent, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import CodexUsagePanel from "./CodexUsagePanel";
import { useCliUsage } from "./CodexUsagePanel/useCliUsage";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
const quota = {
  credentialStatus: "valid",
  success: true,
  tiers: [{ name: "five_hour", utilization: 41.5, resetsAt: null }],
};
function deferred() {
  let resolve!: (value: unknown) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<unknown>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
beforeEach(() => {
  vi.resetAllMocks();
});

describe("local CLI quota and catalog", () => {
  it("shows completed resources independently and does not duplicate refreshes", async () => {
    const claude = deferred();
    const models = deferred();
    invoke.mockImplementation((command) =>
      command === "get_codex_cli_quota"
        ? Promise.resolve(quota)
        : command === "get_claude_cli_quota"
          ? claude.promise
          : models.promise,
    );
    render(<CodexUsagePanel localeText={(zh) => zh} />);
    expect((await screen.findByRole("progressbar", { name: "Codex 5 小时窗口" })).getAttribute("aria-valuenow")).toBe(
      "41.5",
    );
    expect(screen.getByText("正在读取目录…")).toBeTruthy();
    const refresh = screen.getByRole("button", { name: "刷新本机配额与模型" });
    expect(refresh).toHaveProperty("disabled", true);
    fireEvent.click(refresh);
    expect(invoke).toHaveBeenCalledTimes(3);
    await act(async () => {
      models.resolve([{ id: "a" }, { id: "a" }]);
    });
    expect(screen.getByText("目录返回 1 个模型")).toBeTruthy();
    expect(refresh).toHaveProperty("disabled", true);
    await act(async () => {
      claude.resolve({ ...quota, tiers: [] });
    });
    expect(refresh).toHaveProperty("disabled", false);
    expect(screen.getByText("已登录，服务端未返回用量窗口。")).toBeTruthy();
  });

  it("masks each failed query and retains clearly labeled last results", async () => {
    invoke.mockImplementation((command) => Promise.resolve(command === "get_codex_cli_models" ? [{ id: "a" }] : quota));
    render(<CodexUsagePanel localeText={(zh) => zh} />);
    const refresh = await screen.findByRole("button", { name: "刷新本机配额与模型" });
    await waitFor(() => expect(refresh).toHaveProperty("disabled", false));
    invoke.mockRejectedValue(new Error("Authorization Bearer fixture-secret /private/path"));
    fireEvent.click(refresh);
    await waitFor(() => expect(refresh).toHaveProperty("disabled", false));
    expect(screen.getAllByRole("alert")).toHaveLength(3);
    expect(document.body.textContent).not.toContain("fixture-secret");
    expect(document.body.textContent).not.toContain("/private/path");
    expect(screen.getByText("上次读取的目录：1 个模型")).toBeTruthy();
    expect(screen.getAllByText("以下为上次读取结果。")).toHaveLength(2);
    expect(screen.getByRole("progressbar", { name: "Codex 5 小时窗口" }).getAttribute("aria-valuenow")).toBe("41.5");
    invoke.mockImplementation((command) => Promise.resolve(command === "get_codex_cli_models" ? [] : quota));
    fireEvent.click(refresh);
    await waitFor(() => expect(screen.queryAllByRole("alert")).toHaveLength(0));
    expect(screen.getByText("目录返回 0 个模型")).toBeTruthy();
  });

  it.each(["not_found", "expired", "parse_error", "query_error"])(
    "shows the actionable %s state without exposing credential details",
    async (status) => {
      invoke.mockImplementation((command) =>
        Promise.resolve(
          command === "get_codex_cli_models"
            ? []
            : {
                credentialStatus: status,
                success: false,
                tiers: [],
                error: "fixture-secret",
                credentialMessage: "fixture-secret",
              },
        ),
      );
      render(<CodexUsagePanel localeText={(zh) => zh} />);
      await waitFor(() =>
        expect(screen.getByRole("button", { name: "刷新本机配额与模型" })).toHaveProperty("disabled", false),
      );
      const section = screen.getByRole("region", { name: "Codex 订阅用量" });
      const expected =
        status === "not_found"
          ? /未检测到 Codex/
          : status === "expired"
            ? /认证已失效/
            : status === "parse_error"
              ? /无法读取本机凭据/
              : /暂时无法查询/;
      expect(within(section).getByText(expected)).toBeTruthy();
      expect(document.body.textContent).not.toContain("fixture-secret");
    },
  );

  it("rejects invalid containers and filters non-finite quota values", async () => {
    invoke.mockImplementation((command) =>
      Promise.resolve(
        command === "get_codex_cli_models"
          ? { error: "fixture-secret" }
          : command === "get_claude_cli_quota"
            ? null
            : { ...quota, tiers: [null, { name: "bad", utilization: NaN }, { name: "five_hour", utilization: 200 }] },
      ),
    );
    render(<CodexUsagePanel localeText={(zh) => zh} />);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "刷新本机配额与模型" })).toHaveProperty("disabled", false),
    );
    expect(screen.getAllByRole("alert")).toHaveLength(2);
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("100");
    expect(document.body.textContent).not.toContain("NaN");
    expect(document.body.textContent).not.toContain("fixture-secret");
  });

  it("ignores retired mount results and leaves fresh requests usable", async () => {
    const old = deferred();
    invoke
      .mockImplementationOnce(() => old.promise)
      .mockImplementationOnce(() => old.promise)
      .mockImplementationOnce(() => old.promise)
      .mockImplementation((command) => Promise.resolve(command === "get_codex_cli_models" ? [{ id: "new" }] : quota));
    const hook = renderHook(() => useCliUsage(), { wrapper: StrictMode });
    await waitFor(() => expect(hook.result.current.loading).toBe(false));
    expect(hook.result.current.models.data).toEqual([{ id: "new" }]);
    await act(async () => {
      old.resolve({ credentialStatus: "expired", success: false, tiers: [] });
    });
    expect(hook.result.current.codex.data).toEqual(quota);
    expect(hook.result.current.models.data).toEqual([{ id: "new" }]);
    hook.unmount();
  });
});
