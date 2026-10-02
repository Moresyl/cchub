export type ProfileMoveDirection = "up" | "down" | "first" | "last";

export function profileMoveTarget(index: number, count: number, direction: ProfileMoveDirection): number | null {
  if (index < 0 || index >= count) return null;
  const target = direction === "first" ? 0 : direction === "last" ? count - 1 : index + (direction === "up" ? -1 : 1);
  return target < 0 || target >= count || target === index ? null : target;
}

export function reorderedProfileIds(ids: readonly string[], sourceId: string, targetId: string): string[] | null {
  const source = ids.indexOf(sourceId);
  const target = ids.indexOf(targetId);
  if (source < 0 || target < 0 || source === target || new Set(ids).size !== ids.length) return null;
  const next = [...ids];
  const [moved] = next.splice(source, 1);
  next.splice(target, 0, moved);
  return next;
}
