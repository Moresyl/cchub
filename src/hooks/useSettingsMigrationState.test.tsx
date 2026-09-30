import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSettingsMigrationState } from "./useSettingsMigrationState";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("../components/AppDialogProvider", () => ({ useAppDialog: () => ({ confirm: vi.fn() }) }));

const roots = [{ project_root: "remote/project", file_count: 2 }];
const summary = {
  imported_at: "2026-10-01T00:00:00Z",
  db_rows_restored: 5,
  tool_configs_restored: 1,
  skills_restored: 0,
  full_files_restored: 0,
  pending_project_files: 2,
  safety_backup_path: "local/safety.db",
};
const tools = [{ id: "claude", name: "Claude Code" }];
const paths = [{ tool_id: "claude", config_dir: "local/tool", mcp_config_path: null, skills_dir: null }];

beforeEach(() => {
  vi.clearAllMocks();
  invokeMock.mockImplementation(async (command: string) => {
    switch (command) {
      case "detect_tools":
        return tools;
      case "get_custom_paths":
        return paths;
      case "get_tool_environment_report":
        return [];
      case "get_pending_imported_project_roots":
        return roots;
      case "get_last_import_summary":
        return summary;
      case "list_managed_backups":
        return [];
      default:
        throw new Error(`unexpected command: ${command}`);
    }
  });
});

function mount() {
  const client = new QueryClient();
  const setTools = vi.fn();
  const setCustomPaths = vi.fn();
  const hook = renderHook(
    () =>
      useSettingsMigrationState({
        enabled: false,
        locale: "zh",
        settingsText: {},
        toolsLength: 0,
        loadToolsAndPaths: vi.fn(async () => {}),
        setTools,
        setCustomPaths,
        openInSystemWithLabel: vi.fn(),
      }),
    {
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      ),
    },
  );
  return { ...hook, setTools, setCustomPaths, client };
}

describe("settings after a committed backup restore", () => {
  it("updates summary, local bindings and pending projects without resynchronizing native configuration", async () => {
    const hook = mount();
    await act(async () => hook.result.current.handleBackupRestored());
    expect(hook.result.current.lastImportSummary).toEqual(summary);
    expect(hook.result.current.pendingProjectRoots).toEqual(roots);
    expect(hook.result.current.remapTargets).toEqual({ "remote/project": "" });
    expect(hook.result.current.migrationPanelsOpen.summary).toBe(true);
    expect(hook.result.current.migrationPanelsOpen.pending).toBe(true);
    expect(hook.setTools).toHaveBeenCalledWith(tools);
    expect(hook.setCustomPaths).toHaveBeenCalledWith(paths);
    expect(invokeMock.mock.calls.some(([command]) => command === "sync_config_profiles")).toBe(false);
    hook.unmount();
    hook.client.clear();
  });

  it("does not mix partial migration state into the last successful summary", async () => {
    const hook = mount();
    await act(async () => hook.result.current.handleBackupRestored());
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_last_import_summary") throw new Error("summary unavailable");
      return [];
    });
    await act(async () => {
      await expect(hook.result.current.handleBackupRestored()).rejects.toThrow("summary unavailable");
    });
    expect(hook.result.current.lastImportSummary).toEqual(summary);
    expect(hook.result.current.pendingProjectRoots).toEqual(roots);
    expect(hook.setTools).toHaveBeenCalledTimes(1);
    hook.unmount();
    hook.client.clear();
  });
});
