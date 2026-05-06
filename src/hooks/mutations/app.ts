import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys } from "../queries";

export interface SetWelcomeCompletedInput {
  completed: boolean;
}

function invalidateAppPreferences(queryClient: QueryClient) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.welcomeCompleted });
}

export function useSetWelcomeCompletedMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: SetWelcomeCompletedInput) => invoke("set_welcome_completed", { ...input }),
    onSuccess: () => invalidateAppPreferences(queryClient),
  });
}
