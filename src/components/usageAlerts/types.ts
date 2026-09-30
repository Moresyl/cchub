export interface AlertSettings {
  enabled: boolean;
  quotaPercent: number | null;
  balances: { unit: string; amount: number }[];
  systemNotifications: boolean;
}

export interface AlertRule {
  profileId: string;
  queryIdentity: string | null;
  settings: AlertSettings;
  paused: boolean;
  status: string;
  checkedAt: number | null;
}

export interface AlertEvent {
  id: string;
  profileId: string;
  profileName: string;
  toolId: string;
  kind: string;
  label: string;
  value: number;
  threshold: number;
  unit: string | null;
  resetAt: number | null;
  createdAt: number;
  read: boolean;
  systemStatus: string;
}

export interface AlertOverview {
  rules: AlertRule[];
  events: AlertEvent[];
  polling: boolean;
}

export const defaultSettings: AlertSettings = {
  enabled: false,
  quotaPercent: null,
  balances: [],
  systemNotifications: false,
};

export function validateSettings(settings: AlertSettings): boolean {
  const quota = settings.quotaPercent;
  if (quota !== null && (!Number.isFinite(quota) || quota < 1 || quota > 100)) return false;
  const units = settings.balances.map(({ unit }) => unit.trim().toLowerCase());
  if (units.length > 10 || new Set(units).size !== units.length) return false;
  if (
    settings.balances.some(
      ({ unit, amount }) =>
        !unit.trim() ||
        new TextEncoder().encode(unit.trim()).length > 32 ||
        [...unit].some((character) => {
          const code = character.charCodeAt(0);
          return code < 32 || (code >= 127 && code <= 159);
        }) ||
        !Number.isFinite(amount) ||
        amount < 0,
    )
  )
    return false;
  return !settings.enabled || quota !== null || units.length > 0;
}
