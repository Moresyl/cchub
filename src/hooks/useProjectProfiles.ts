import { useEffect, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface ProjectProfile {
  id: string;
  name: string;
  description: string | null;
  snapshot: { version: number; workspaceId: string | null; configProfileIds: string[] };
  updatedAt: string;
  lastAppliedAt: string | null;
  isActive: boolean;
}

type Action = "create" | "apply" | "update" | "delete";
interface State {
  profiles: ProjectProfile[];
  loading: boolean;
  pending: string | null;
  error: "read" | "mutation" | "refresh" | null;
  committed: Action | null;
}

const refreshEvent = "cchub-project-profile-refresh";
let state: State = { profiles: [], loading: true, pending: null, error: null, committed: null };
let generation = 0;
let reading: Promise<boolean> | null = null;
const listeners = new Set<() => void>();

function publish(next: Partial<State>) {
  state = { ...state, ...next };
  for (const listener of listeners) listener();
}

async function load(allowPending = false, supersede = false): Promise<boolean> {
  if (state.pending && !allowPending) return false;
  if (reading && !supersede) return reading;
  const attempt = ++generation;
  publish({ loading: true });
  const request = (async () => {
    try {
      const profiles = await invoke<ProjectProfile[]>("get_project_profiles");
      if (generation !== attempt) return false;
      publish({ profiles, error: null });
      return true;
    } catch {
      if (generation === attempt) publish({ error: state.committed ? "refresh" : "read" });
      return false;
    } finally {
      if (generation === attempt) {
        reading = null;
        publish({ loading: false });
      }
    }
  })();
  reading = request;
  return request;
}

function onExternalRefresh(event: Event) {
  if (event instanceof CustomEvent && event.detail === "project-profile-store") return;
  void load(false, true);
}

function subscribe(listener: () => void) {
  if (!listeners.size) window.addEventListener(refreshEvent, onExternalRefresh);
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (!listeners.size) window.removeEventListener(refreshEvent, onExternalRefresh);
  };
}

function sameSnapshot(left: ProjectProfile, right: ProjectProfile) {
  return (
    left.snapshot.version === right.snapshot.version &&
    left.snapshot.workspaceId === right.snapshot.workspaceId &&
    [...new Set(left.snapshot.configProfileIds)].sort().join("\0") ===
      [...new Set(right.snapshot.configProfileIds)].sort().join("\0")
  );
}

async function mutate(action: Action, args: Record<string, unknown>): Promise<boolean> {
  // Shared by the panel and title-bar switcher, even before React renders.
  if (state.pending || state.loading || state.error) return false;
  generation++;
  reading = null;
  publish({ pending: typeof args.id === "string" ? args.id : "create", error: null, committed: null });
  try {
    const result = await invoke<ProjectProfile | { profile: ProjectProfile; appliedProfileIds: string[] } | void>(
      `${action}_project_profile`,
      args,
    );
    if (action === "delete") {
      publish({ profiles: state.profiles.filter((profile) => profile.id !== args.id) });
    } else if (result) {
      const profile = "profile" in result ? result.profile : result;
      let profiles = state.profiles.filter((item) => item.id !== profile.id);
      if (action === "apply") {
        profiles = profiles.map((item) => ({ ...item, isActive: profile.isActive && sameSnapshot(item, profile) }));
      }
      publish({ profiles: [profile, ...profiles] });
    }
    publish({ committed: action });
    window.dispatchEvent(new CustomEvent(refreshEvent, { detail: "project-profile-store" }));
    await load(true);
    return true;
  } catch {
    publish({ error: "mutation" });
    return false;
  } finally {
    publish({ pending: null });
  }
}

const getState = () => state;
const refresh = () => load();

export function useProjectProfiles() {
  const snapshot = useSyncExternalStore(subscribe, getState);
  useEffect(() => {
    void load();
  }, []);
  return { ...snapshot, refresh, mutate };
}
