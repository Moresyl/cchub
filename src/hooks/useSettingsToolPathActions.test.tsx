import { act, renderHook } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { useSettingsToolPathActions } from "./useSettingsToolPathActions";
import type { SettingsToolPathCardCustomPath } from "../components/SettingsToolPathCard";

const path = { tool_id: "codex", config_dir: "config", mcp_config_path: "custom-mcp", skills_dir: "custom-skills" };
function mount() {
  const save = vi.fn(async () => {});
  const pickFile = vi.fn(async () => null as string | null);
  const pickFolder = vi.fn(async () => null as string | null);
  const hook = renderHook(() => {
    const [customPaths, setCustomPaths] = useState<SettingsToolPathCardCustomPath[]>([path]);
    return {
      customPaths,
      setCustomPaths,
      ...useSettingsToolPathActions({ customPaths, setCustomPaths, save, pickFile, pickFolder }),
    };
  });
  return { ...hook, save, pickFile, pickFolder };
}
describe("canonical settings path writes", () => {
  it("normalizes edits, retains unrelated fields and uses the acknowledged row on subsequent writes", async () => {
    const h = mount();
    await act(async () => {
      expect(await h.result.current.onSaveMcpPath("codex", "  new-mcp  ", "default-mcp")).toBe("new-mcp");
    });
    expect(h.save).toHaveBeenLastCalledWith("codex", "config", "new-mcp", "custom-skills");
    await act(async () => {
      await h.result.current.onSaveSkillsDir("codex", "new-skills", "default-skills");
    });
    expect(h.save).toHaveBeenLastCalledWith("codex", "config", "new-mcp", "new-skills");
    h.unmount();
  });
  it("clears only the requested override on empty/default input and skips unchanged writes", async () => {
    const h = mount();
    await act(async () => {
      expect(await h.result.current.onSaveMcpPath("codex", " ", "default-mcp")).toBe("default-mcp");
    });
    expect(h.save).toHaveBeenLastCalledWith("codex", "config", null, "custom-skills");
    await act(async () => {
      await h.result.current.onSaveMcpPath("codex", "default-mcp", "default-mcp");
    });
    expect(h.save).toHaveBeenCalledTimes(1);
    await act(async () => {
      await h.result.current.onSaveSkillsDir("codex", "default-skills", "default-skills");
    });
    expect(h.save).toHaveBeenLastCalledWith("codex", "config", null, null);
    h.unmount();
  });
  it("reads unrelated fields after a chooser finishes and cancellation performs no write", async () => {
    const h = mount();
    let finish!: (value: string) => void;
    h.pickFile.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    let action!: Promise<string | null>;
    act(() => {
      action = h.result.current.onPickMcpPath("codex");
    });
    act(() => h.result.current.setCustomPaths([{ ...path, skills_dir: "newer-skills" }]));
    await act(async () => {
      finish("chosen-mcp");
      await action;
    });
    expect(h.save).toHaveBeenLastCalledWith("codex", "config", "chosen-mcp", "newer-skills");
    await act(async () => {
      expect(await h.result.current.onPickSkillsDir("codex")).toBeNull();
    });
    expect(h.save).toHaveBeenCalledTimes(1);
    h.unmount();
  });
  it("does not acknowledge failed writes or damage another tool row", async () => {
    const h = mount();
    h.save.mockRejectedValueOnce(new Error("disk unavailable"));
    await act(async () => {
      await expect(h.result.current.onSaveMcpPath("codex", "failed", "default")).rejects.toThrow("disk unavailable");
    });
    expect(h.result.current.customPaths).toEqual([path]);
    await act(async () => {
      await h.result.current.onSaveSkillsDir("claude", "claude-skills", "default");
    });
    expect(h.result.current.customPaths).toEqual([
      path,
      { tool_id: "claude", config_dir: null, mcp_config_path: null, skills_dir: "claude-skills" },
    ]);
    h.unmount();
  });
});
