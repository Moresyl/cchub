export const DEFAULT_SIDEBAR_WIDTH = 264;
export const MIN_SIDEBAR_WIDTH = 264;
export const MAX_SIDEBAR_WIDTH = 360;

export function clampSidebarWidth(width: number): number {
  if (!Number.isFinite(width)) return DEFAULT_SIDEBAR_WIDTH;
  return Math.min(MAX_SIDEBAR_WIDTH, Math.max(MIN_SIDEBAR_WIDTH, Math.round(width)));
}

export function readSidebarWidth(): number {
  try {
    const saved = window.localStorage.getItem("cchub:sidebar-width");
    return saved === null ? DEFAULT_SIDEBAR_WIDTH : clampSidebarWidth(Number(saved));
  } catch {
    return DEFAULT_SIDEBAR_WIDTH;
  }
}
