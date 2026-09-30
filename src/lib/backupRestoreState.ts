import type { QueryClient } from "@tanstack/react-query";
import { queryClient } from "./queryClient";

// Restore replaces the database behind every cached configuration query.
// Cancel earlier reads first, and remove inactive data because this app does
// not refetch cached queries on mount. Refresh failures cannot undo a restore.
export async function refreshBackupRestoreState(
  refreshLocalState?: () => Promise<void>,
  client: QueryClient = queryClient,
): Promise<boolean> {
  const cancellation = await Promise.allSettled([client.cancelQueries()]);
  client.removeQueries({ type: "inactive" });
  const refresh = await Promise.allSettled([
    client.resetQueries({ type: "active" }, { throwOnError: true }),
    refreshLocalState ? Promise.resolve().then(refreshLocalState) : undefined,
  ]);
  return [...cancellation, ...refresh].every((result) => result.status === "fulfilled");
}
