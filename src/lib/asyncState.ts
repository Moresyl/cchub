import { useCallback, useEffect, useRef, useState } from "react";

export interface AsyncState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
}

export type AsyncResource<T> = AsyncState<T> & {
  reload: () => void;
};

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return "Unknown error";
}

export function useAsyncResource<T>(loader: () => Promise<T>): AsyncResource<T> {
  const loaderRef = useRef(loader);
  const requestIdRef = useRef(0);
  const [state, setState] = useState<AsyncState<T>>({
    data: null,
    loading: true,
    error: null,
  });

  useEffect(() => {
    loaderRef.current = loader;
  }, [loader]);

  const reload = useCallback(() => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    setState((current) => ({ ...current, loading: true, error: null }));

    void loaderRef
      .current()
      .then((data) => {
        if (requestIdRef.current !== requestId) {
          return;
        }
        setState({ data, loading: false, error: null });
      })
      .catch((error: unknown) => {
        if (requestIdRef.current !== requestId) {
          return;
        }
        setState((current) => ({
          data: current.data,
          loading: false,
          error: toErrorMessage(error),
        }));
      });
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  return { ...state, reload };
}
