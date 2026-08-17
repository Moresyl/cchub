export interface IdleTaskOptions {
  delay?: number;
  timeout?: number;
}

/**
 * Delay non-critical work until after the first interaction window, then let
 * the browser run it during an idle period. The returned function cancels both
 * the delay and a queued idle callback.
 */
export function scheduleIdleTask(task: () => void, options: IdleTaskOptions = {}): () => void {
  const { delay = 1_000, timeout = 5_000 } = options;
  let idleId: number | null = null;
  let cancelled = false;

  const delayId = window.setTimeout(
    () => {
      if (cancelled) return;

      if (typeof window.requestIdleCallback === "function") {
        idleId = window.requestIdleCallback(
          () => {
            idleId = null;
            if (!cancelled) task();
          },
          { timeout },
        );
        return;
      }

      task();
    },
    Math.max(0, delay),
  );

  return () => {
    cancelled = true;
    window.clearTimeout(delayId);
    if (idleId !== null && typeof window.cancelIdleCallback === "function") {
      window.cancelIdleCallback(idleId);
    }
  };
}
