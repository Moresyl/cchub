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

export function cloudSettingsChanged<T extends object>(current: T, saved: T | null, keys: readonly (keyof T)[]) {
  return saved !== null && keys.some((key) => current[key] !== saved[key]);
}
