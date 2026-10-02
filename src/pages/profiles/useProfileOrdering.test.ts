import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useProfileOrdering } from "./useProfileOrdering";
import type { ConfigProfile } from "./helpers";

const mutateAsync = vi.fn();
const toast = vi.fn();
vi.mock("../../hooks/mutations/profile", () => ({
  useReorderConfigProfilesMutation: () => ({ mutateAsync }),
}));
vi.mock("../../components/Toast", () => ({ showToast: (...args: unknown[]) => toast(...args) }));

function profile(id: string, sort_order = 0, tool_id = "claude"): ConfigProfile {
  return { id, name: id.toUpperCase(), tool_id, sort_order, config_snapshot: "{}", created_at: null, updated_at: null };
}

function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function setup(
  options: { enabled?: boolean; filterTool?: string; displayed?: string[]; locale?: "zh" | "en" | "ja" } = {},
) {
  const reload = vi.fn().mockResolvedValue(undefined);
  const hook = renderHook(() => {
    const [profiles, setProfiles] = useState([
      profile("a", 2),
      profile("b", 2),
      profile("c", 2),
      profile("other", 9, "codex"),
    ]);
    const orderedProfiles = options.displayed
      ? options.displayed.map((id) => profiles.find((item) => item.id === id)!)
      : profiles
          .filter((item) => item.tool_id === "claude")
          .sort((a, b) => a.sort_order - b.sort_order || b.name.localeCompare(a.name));
    const ordering = useProfileOrdering({
      profiles,
      orderedProfiles,
      filterTool: options.filterTool ?? "claude",
      enabled: options.enabled ?? true,
      setProfiles,
      reload,
      localeText: (zh, en, ja) => (options.locale === "en" ? en : options.locale === "ja" ? ja! : zh),
    });
    return { ...ordering, profiles, setProfiles };
  });
  return { ...hook, reload };
}

beforeEach(() => {
  vi.clearAllMocks();
  mutateAsync.mockResolvedValue(undefined);
});

describe("useProfileOrdering", () => {
  it("saves displayed tie order and blocks repeated operations until the first save finishes", async () => {
    const pending = deferred();
    mutateAsync.mockReturnValue(pending.promise);
    const { result } = setup();
    let operation!: Promise<void>;
    act(() => {
      operation = result.current.reorderProfiles("c", "a");
      void result.current.reorderProfiles("b", "a");
    });
    expect(mutateAsync).toHaveBeenCalledExactlyOnceWith({ toolId: "claude", orderedIds: ["b", "a", "c"] });
    expect(result.current.orderBusy).toBe(true);
    expect(result.current.profiles.map((item) => [item.id, item.sort_order])).toEqual([
      ["a", 1],
      ["b", 0],
      ["c", 2],
      ["other", 9],
    ]);
    await act(async () => {
      pending.resolve();
      await operation;
    });
    expect(result.current.orderBusy).toBe(false);
    expect(result.current.orderAnnouncement).toBe("已将“C”移至第 3 位");
    await act(() => result.current.reorderProfiles("c", "b"));
    expect(mutateAsync).toHaveBeenCalledTimes(2);
  });

  it("rolls back only its positions and retains edits, newly added profiles and other tools", async () => {
    const pending = deferred();
    mutateAsync.mockReturnValue(pending.promise);
    const { result, reload } = setup();
    let operation!: Promise<void>;
    act(() => {
      operation = result.current.reorderProfiles("c", "a");
    });
    act(() =>
      result.current.setProfiles((current) => [
        ...current.map((item) =>
          item.id === "a"
            ? { ...item, name: "Renamed", config_snapshot: "changed" }
            : item.id === "b"
              ? { ...item, sort_order: 77 }
              : item.id === "other"
                ? { ...item, sort_order: 33 }
                : item,
        ),
        profile("new", 8),
      ]),
    );
    await act(async () => {
      pending.reject(new Error("disk unavailable"));
      await operation;
    });
    expect(result.current.profiles.map((item) => [item.id, item.sort_order])).toEqual([
      ["a", 2],
      ["b", 77],
      ["c", 2],
      ["other", 33],
      ["new", 8],
    ]);
    expect(result.current.profiles[0]).toMatchObject({ name: "Renamed", config_snapshot: "changed" });
    expect(toast).toHaveBeenCalledWith("error", "排序未保存：Error: disk unavailable");
    expect(reload).toHaveBeenCalledTimes(1);
    expect(result.current.orderBusy).toBe(false);
    expect(result.current.orderAnnouncement).toBe("");
  });

  it("releases the operation guard even when recovery fails", async () => {
    const { result, reload } = setup();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    try {
      mutateAsync.mockRejectedValueOnce(new Error("save failed"));
      reload.mockRejectedValueOnce(new Error("refresh failed"));
      await act(() => result.current.reorderProfiles("c", "a"));
      expect(result.current.orderBusy).toBe(false);
      await act(() => result.current.reorderProfiles("c", "b"));
      expect(mutateAsync).toHaveBeenCalledTimes(2);
    } finally {
      warn.mockRestore();
    }
  });

  it.each([{ enabled: false }, { filterTool: "" }, { displayed: ["a", "other", "c"] }, { displayed: ["a", "a", "c"] }])(
    "ignores an unavailable or invalid ordering context %j",
    async (options) => {
      const { result } = setup(options);
      await act(() => result.current.reorderProfiles("a", "c"));
      expect(mutateAsync).not.toHaveBeenCalled();
    },
  );

  it("ignores unknown rows and boundary moves", async () => {
    const { result } = setup();
    await act(async () => {
      await result.current.reorderProfiles("missing", "a");
      await result.current.reorderProfiles("a", "a");
      result.current.handleMoveProfile("c", "first");
      result.current.handleMoveProfile("a", "down");
      result.current.handleMoveProfile("missing", "last");
    });
    expect(mutateAsync).not.toHaveBeenCalled();
  });

  it.each(["zh", "en", "ja"] as const)("announces a keyboard move in %s", async (locale) => {
    const { result } = setup({ locale });
    await act(async () => {
      result.current.handleMoveProfile("a", "first");
    });
    expect(mutateAsync).toHaveBeenCalledWith({ toolId: "claude", orderedIds: ["a", "c", "b"] });
    expect(result.current.orderAnnouncement).toBe(
      locale === "zh"
        ? "已将“A”移至第 1 位"
        : locale === "en"
          ? "Moved “A” to position 1"
          : "「A」を 1 番目に移動しました",
    );
  });
});
