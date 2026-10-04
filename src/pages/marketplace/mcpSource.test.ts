import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { resolveInstalledMcp } from "./mcpSource";
import type { InstalledMcpServer, RegistryEntry } from "./helpers";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.resetAllMocks());
const entry = { id: "market", name: "same" } as RegistryEntry;
const servers = ["one", "two"].map(
  (id): InstalledMcpServer => ({
    id,
    name: "same",
    origin: { native_name: "same", revision: "rev", bindings: [] },
    command: "node",
    args: "[]",
    env: "{}",
    status: "active",
    transport: "stdio",
    source: "local",
    package_name: null,
    version: null,
    config_path: null,
  }),
);

it("resolves the selected tool's source even when another tool has an equal name", async () => {
  vi.mocked(invoke).mockResolvedValue({
    one: { gemini: { state: "unowned", disabled: false } },
    two: { gemini: { state: "source", disabled: true } },
  });
  expect(await resolveInstalledMcp(entry, "gemini", servers)).toBe(servers[1]);
  expect(invoke).toHaveBeenCalledWith("get_mcp_sync_statuses", { serverIds: ["one", "two"] });
});

it.each(["conflict", "missing", "unowned"])(
  "refuses a %s source rather than choosing a same-name entry",
  async (state) => {
    vi.mocked(invoke).mockResolvedValue({
      one: { gemini: { state, disabled: false } },
      two: { gemini: { state, disabled: false } },
    });
    await expect(resolveInstalledMcp(entry, "gemini", servers)).rejects.toThrow("unambiguous");
  },
);

it("does not grant write authority after a failed status read", async () => {
  vi.mocked(invoke).mockRejectedValue(new Error("unavailable"));
  await expect(resolveInstalledMcp(entry, "gemini", servers)).rejects.toThrow();
});
