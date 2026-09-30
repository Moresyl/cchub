function normalizedUrl(value: string) {
  try {
    return new URL(value.trim()).toString().replace(/\/+$/, "");
  } catch {
    return value.trim().replace(/\/+$/, "");
  }
}

export function sameWebDavAccount(
  left: { base_url: string; username: string },
  right: { base_url: string; username: string },
) {
  return (
    normalizedUrl(left.base_url) === normalizedUrl(right.base_url) && left.username.trim() === right.username.trim()
  );
}

export function sameS3Account(
  left: { endpoint: string; region: string; accessKeyId: string },
  right: { endpoint: string; region: string; accessKeyId: string },
) {
  const endpoint = (settings: typeof left) =>
    normalizedUrl(settings.endpoint.trim() || `https://s3.${settings.region.trim() || "us-east-1"}.amazonaws.com`);
  return endpoint(left) === endpoint(right) && left.accessKeyId.trim() === right.accessKeyId.trim();
}

function normalizedSegment(value: string, fallback: string) {
  return value.trim().replace(/^\/+|\/+$/g, "") || fallback;
}

export function sameWebDavBackupLocation(
  left: { base_url: string; username: string; remote_root: string; profile: string },
  right: typeof left,
) {
  return (
    sameWebDavAccount(left, right) &&
    normalizedSegment(left.remote_root, "cchub-sync") === normalizedSegment(right.remote_root, "cchub-sync") &&
    normalizedSegment(left.profile, "default") === normalizedSegment(right.profile, "default")
  );
}

export function sameS3BackupLocation(
  left: { endpoint: string; region: string; accessKeyId: string; bucket: string; remoteRoot: string; profile: string },
  right: typeof left,
) {
  return (
    sameS3Account(left, right) &&
    left.bucket.trim() === right.bucket.trim() &&
    normalizedSegment(left.remoteRoot, "cchub-sync") === normalizedSegment(right.remoteRoot, "cchub-sync") &&
    normalizedSegment(left.profile, "default") === normalizedSegment(right.profile, "default")
  );
}

export function cloudSettingsChanged<T extends object>(current: T, saved: T | null, keys: readonly (keyof T)[]) {
  return saved !== null && keys.some((key) => current[key] !== saved[key]);
}
