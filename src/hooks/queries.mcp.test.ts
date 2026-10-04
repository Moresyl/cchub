import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { queryClient } from "../lib/queryClient";
import { fetchMarketplaceLocalData, queryKeys } from "./queries";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => {
  queryClient.clear();
  vi.resetAllMocks();
});

it("refreshes native revisions rather than editing an indefinitely cached marketplace source", async () => {
  queryClient.setQueryData(queryKeys.mcpServersPage, { servers: [{ id: "old" }], tools: [] });
  queryClient.setQueryData(queryKeys.skillsPage, { skills: [] });
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "scan_mcp_servers" ? [{ id: "fresh", origin: { revision: "new" } }] : [],
  );
  const result = await fetchMarketplaceLocalData();
  expect(result.servers).toEqual([{ id: "fresh", origin: { revision: "new" } }]);
  expect(invoke).toHaveBeenCalledWith("scan_mcp_servers");
});
