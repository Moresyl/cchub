import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";
import type { Update } from "@tauri-apps/plugin-updater";

type AppUpdateSource = "tauri" | "github";

export interface UpdaterEnvironmentState {
  disabled_by_env: boolean;
  env_var_value: string | null;
}

export interface AppUpdateResult {
  update_available: boolean;
  latest_version: string | null;
  current_version: string | null;
  body: string | null;
  not_configured: boolean;
  can_install: boolean;
  release_url: string | null;
  source: AppUpdateSource | null;
  disabled_by_env: boolean;
}

export interface AppUpdateHandle {
  source: AppUpdateSource;
  update?: Update;
  releaseUrl?: string;
}

const RELEASE_API_URL = "https://api.github.com/repos/Moresyl/cchub/releases/latest";

function buildNoUpdateResult(currentVersion: string, overrides?: Partial<AppUpdateResult>): AppUpdateResult {
  return {
    update_available: false,
    latest_version: null,
    current_version: currentVersion || null,
    body: null,
    not_configured: false,
    can_install: false,
    release_url: null,
    source: null,
    disabled_by_env: false,
    ...overrides,
  };
}

function normalizeVersion(version: string | null | undefined): string {
  return String(version ?? "")
    .trim()
    .replace(/^[^\d]*/, "");
}

function compareVersions(left: string, right: string): number {
  const a = normalizeVersion(left)
    .split(".")
    .map((part) => Number.parseInt(part, 10) || 0);
  const b = normalizeVersion(right)
    .split(".")
    .map((part) => Number.parseInt(part, 10) || 0);
  const max = Math.max(a.length, b.length);

  for (let i = 0; i < max; i += 1) {
    const av = a[i] ?? 0;
    const bv = b[i] ?? 0;
    if (av > bv) return 1;
    if (av < bv) return -1;
  }

  return 0;
}

function isUpdaterNotConfigured(message: string): boolean {
  return message.includes("not configured") || message.includes("pubkey");
}

function isRemoteReleaseManifestError(message: string): boolean {
  return (
    message.includes("Could not fetch a valid release JSON from the remote") ||
    message.includes("release JSON") ||
    message.includes("latest.json") ||
    message.includes("404")
  );
}

async function getCurrentAppVersion(): Promise<string> {
  try {
    return await getVersion();
  } catch (error) {
    console.warn("Failed to read current app version", error);
    return "";
  }
}

export async function getUpdaterEnvironmentState(): Promise<UpdaterEnvironmentState> {
  try {
    return await invoke<UpdaterEnvironmentState>("get_updater_environment_state");
  } catch (error) {
    console.warn("Failed to read updater environment state", error);
    return {
      disabled_by_env: false,
      env_var_value: null,
    };
  }
}

async function checkGitHubRelease(currentVersion: string): Promise<{
  result: AppUpdateResult;
  handle: AppUpdateHandle | null;
}> {
  const response = await fetch(RELEASE_API_URL, {
    headers: {
      Accept: "application/vnd.github+json",
    },
  });

  if (!response.ok) {
    throw new Error(`GitHub release request failed: ${response.status} ${response.statusText}`);
  }

  const release = (await response.json()) as {
    tag_name?: string;
    name?: string;
    body?: string;
    html_url?: string;
  };

  const latestVersion = normalizeVersion(release.tag_name || release.name || "");
  if (!latestVersion) {
    throw new Error("GitHub release version not found");
  }

  const hasUpdate = compareVersions(latestVersion, currentVersion) > 0;
  return {
    result: {
      update_available: hasUpdate,
      latest_version: hasUpdate ? latestVersion : null,
      current_version: currentVersion || null,
      body: hasUpdate ? (release.body ?? null) : null,
      not_configured: false,
      can_install: false,
      release_url: release.html_url ?? null,
      source: hasUpdate ? "github" : null,
      disabled_by_env: false,
    },
    handle: hasUpdate
      ? {
          source: "github",
          releaseUrl: release.html_url ?? undefined,
        }
      : null,
  };
}

export async function checkAppUpdate(): Promise<{
  result: AppUpdateResult;
  handle: AppUpdateHandle | null;
}> {
  const currentVersion = await getCurrentAppVersion();
  const updaterEnvironment = await getUpdaterEnvironmentState();

  if (updaterEnvironment.disabled_by_env) {
    return {
      result: buildNoUpdateResult(currentVersion, {
        disabled_by_env: true,
      }),
      handle: null,
    };
  }

  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) {
      try {
        return await checkGitHubRelease(currentVersion);
      } catch (fallbackError) {
        console.warn("GitHub release fallback failed", fallbackError);
        return {
          result: buildNoUpdateResult(currentVersion),
          handle: null,
        };
      }
    }

    return {
      result: {
        update_available: true,
        latest_version: update.version,
        current_version: update.currentVersion ?? (currentVersion || null),
        body: update.body ?? null,
        not_configured: false,
        can_install: true,
        release_url: null,
        source: "tauri",
        disabled_by_env: false,
      },
      handle: {
        source: "tauri",
        update,
      },
    };
  } catch (error) {
    const message = String(error);
    if (isUpdaterNotConfigured(message)) {
      try {
        return await checkGitHubRelease(currentVersion);
      } catch (fallbackError) {
        console.warn("GitHub release fallback failed", fallbackError);
        return {
          result: buildNoUpdateResult(currentVersion, {
            not_configured: true,
          }),
          handle: null,
        };
      }
    }

    if (isRemoteReleaseManifestError(message)) {
      return checkGitHubRelease(currentVersion);
    }

    try {
      return await checkGitHubRelease(currentVersion);
    } catch (fallbackError) {
      console.warn("GitHub release fallback failed", fallbackError);
      throw error;
    }
  }
}

export async function installAppUpdate(handle: AppUpdateHandle): Promise<void> {
  if (handle.source === "tauri" && handle.update) {
    await handle.update.downloadAndInstall();
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
    return;
  }

  if (handle.source === "github" && handle.releaseUrl) {
    await open(handle.releaseUrl);
    return;
  }

  throw new Error("Update target is not available");
}
