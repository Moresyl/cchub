import { lazy, Suspense, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DeepLinkErrorPayload, DeepLinkImportRequest } from "../lib/deeplink";

const DeepLinkImportDialog = lazy(() => import("./DeepLinkImportDialog"));

/** Keep the expensive import UI out of the startup bundle until it is needed. */
export default function DeepLinkImportHost() {
  const [active, setActive] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const activate = () => {
      if (!cancelled) setActive(true);
    };

    const importListener = listen<DeepLinkImportRequest>("deeplink-import", activate);
    const errorListener = listen<DeepLinkErrorPayload>("deeplink-error", activate);
    void invoke<boolean>("has_pending_deeplinks")
      .then((hasPending) => {
        if (hasPending) activate();
      })
      .catch((error) => console.debug("Pending deep link check failed", error));

    return () => {
      cancelled = true;
      void importListener.then((unlisten) => unlisten());
      void errorListener.then((unlisten) => unlisten());
    };
  }, []);

  if (!active) return null;

  return (
    <Suspense fallback={null}>
      <DeepLinkImportDialog />
    </Suspense>
  );
}
