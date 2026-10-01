import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DraftModelTestPanel from "./DraftModelTestPanel";
import { useDraftModelChecks, type DraftModelCheckResult } from "./draftModelChecks";
import { modelCheckMessage } from "./modelCheckMessage";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

beforeEach(() => vi.clearAllMocks());

const props = {
  scope: "saved",
  toolId: "claude",
  snapshot: '{"env":{"ANTHROPIC_AUTH_TOKEN":"draft-key"}}',
  model: "one",
  configuredModels: [],
  catalog: [],
  localeText: (zh: string) => zh,
};
const healthy: DraftModelCheckResult = {
  model: "one",
  status: "healthy",
  httpStatus: 200,
  latencyMs: 12,
  message: "verified",
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe("draft model test panel", () => {
  it("sends the current raw snapshot and a custom test model without saving the profile", async () => {
    invoke.mockResolvedValue([healthy]);
    render(<DraftModelTestPanel {...props} />);
    fireEvent.change(screen.getByLabelText("测试模型"), { target: { value: "new-test-model" } });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "测试选定模型" })));
    expect(invoke).toHaveBeenCalledExactlyOnceWith("test_profile_draft", {
      toolId: "claude",
      configSnapshot: props.snapshot,
      models: ["new-test-model"],
    });
    expect(screen.getByRole("status").textContent).toContain("1/1 个模型通过");
    expect(screen.getByText("模型响应验证通过 · HTTP 200 · 12 ms")).toBeTruthy();
  });

  it("tests exactly the configured models, not the entire discovered catalog", async () => {
    invoke.mockResolvedValue([
      healthy,
      { ...healthy, model: "two", status: "error", httpStatus: 401, message: "Authentication denied" },
    ]);
    render(
      <DraftModelTestPanel
        {...props}
        configuredModels={["one", " one ", "two", ""]}
        catalog={[{ id: "catalog-only" }]}
      />,
    );
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "测试配置中的 2 个模型" })));
    expect(invoke).toHaveBeenCalledWith("test_profile_draft", expect.objectContaining({ models: ["one", "two"] }));
    expect(screen.getByRole("status").textContent).toContain("1/2 个模型通过");
    expect(screen.getByText("认证失败，请检查当前草稿的密钥或登录状态 · HTTP 401 · 12 ms")).toBeTruthy();
  });

  it.each(['{"env":', "[]", "null"])("blocks invalid raw configuration %s", (snapshot) => {
    render(<DraftModelTestPanel {...props} snapshot={snapshot} />);
    expect(screen.getByRole("button", { name: "测试选定模型" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("alert").textContent).toContain("JSON");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("shows test errors without closing the editor or losing its selected model", async () => {
    invoke.mockRejectedValue(new Error("network unavailable"));
    render(<DraftModelTestPanel {...props} />);
    fireEvent.change(screen.getByLabelText("测试模型"), { target: { value: "custom" } });
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "测试选定模型" })));
    expect(screen.getByRole("alert").textContent).toContain("network unavailable");
    expect(screen.getByLabelText("测试模型")).toHaveProperty("value", "custom");
    expect(screen.getByRole("button", { name: "测试选定模型" })).toHaveProperty("disabled", false);
  });

  it("blocks oversized batches and empty selected model IDs", () => {
    render(
      <DraftModelTestPanel {...props} model="" configuredModels={Array.from({ length: 33 }, (_, i) => `model-${i}`)} />,
    );
    expect(screen.getByRole("button", { name: "测试配置中的 33 个模型" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "测试选定模型" })).toHaveProperty("disabled", true);
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("draft model check ownership", () => {
  it.each(["scope", "tool", "snapshot", "selection"])(
    "rejects stale results and cleanup after %s changes",
    async (field) => {
      const old = deferred<DraftModelCheckResult[]>();
      const next = deferred<DraftModelCheckResult[]>();
      invoke.mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
      const initial = { scope: "saved", tool: "claude", snapshot: props.snapshot, selection: "one" };
      const hook = renderHook(
        ({ scope, tool, snapshot, selection }) => useDraftModelChecks(scope, tool, snapshot, selection),
        { initialProps: initial },
      );
      let first!: Promise<void>;
      act(() => {
        first = hook.result.current.run(["one"]);
      });
      hook.rerender({ ...initial, [field]: field === "snapshot" ? '{"key":"new-key","account":"other"}' : "other" });
      let second!: Promise<void>;
      act(() => {
        second = hook.result.current.run(["two"]);
      });
      await act(async () => {
        old.resolve([healthy]);
        await first;
      });
      expect(hook.result.current.results).toEqual([]);
      expect(hook.result.current.running).toBe(true);
      await act(async () => {
        next.resolve([{ ...healthy, model: "two" }]);
        await second;
      });
      expect(hook.result.current.results[0].model).toBe("two");
      expect(hook.result.current.running).toBe(false);
    },
  );

  it("prevents duplicate clicks before React updates the loading state", async () => {
    const response = deferred<DraftModelCheckResult[]>();
    invoke.mockReturnValue(response.promise);
    const hook = renderHook(() => useDraftModelChecks("saved", "claude", props.snapshot, "one"));
    let pending!: Promise<void>;
    act(() => {
      pending = hook.result.current.run(["one"]);
      void hook.result.current.run(["one"]);
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    await act(async () => {
      response.resolve([healthy]);
      await pending;
    });
    expect(hook.result.current.results).toEqual([healthy]);
  });

  it("ignores stale errors and unmounted requests", async () => {
    const response = deferred<DraftModelCheckResult[]>();
    invoke.mockReturnValue(response.promise);
    const hook = renderHook(({ snapshot }) => useDraftModelChecks("saved", "claude", snapshot, "one"), {
      initialProps: { snapshot: props.snapshot },
    });
    let pending!: Promise<void>;
    act(() => {
      pending = hook.result.current.run(["one"]);
    });
    hook.rerender({ snapshot: '{"key":"changed"}' });
    await act(async () => {
      response.reject(new Error("stale failure"));
      await pending;
    });
    expect(hook.result.current.error).toBeNull();
    const next = deferred<DraftModelCheckResult[]>();
    invoke.mockReturnValue(next.promise);
    act(() => {
      pending = hook.result.current.run(["one"]);
    });
    hook.unmount();
    await act(async () => {
      next.resolve([healthy]);
      await pending;
    });
  });
});

describe("model check messages", () => {
  it.each([
    [401, "", "认证失败"],
    [403, "", "没有访问权限"],
    [404, "", "不存在"],
    [429, "", "额度不足"],
    [503, "", "供应商服务"],
    [null, "Model request timed out", "超时"],
    [200, "Model stream was interrupted", "Model stream was interrupted"],
  ] as const)("describes HTTP %s failures accurately", (httpStatus, message, expected) => {
    expect(modelCheckMessage({ ...healthy, status: "error", httpStatus, message }, (zh) => zh)).toContain(expected);
  });
  it("localizes successful results", () => {
    expect(modelCheckMessage(healthy, (_zh, en) => en)).toBe("Model reply verified");
  });
});
