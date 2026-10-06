export function isNativeOpenCodeConfig(snapshot: string): boolean {
  try {
    const value = JSON.parse(snapshot);
    if (!value || typeof value !== "object" || Array.isArray(value)) return false;
    return (
      value.metadata?.nativeFormat === "providers" ||
      ["providers", "package", "canonical", "settings"].some((key) => Object.prototype.hasOwnProperty.call(value, key))
    );
  } catch {
    return false;
  }
}
