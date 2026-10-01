export type RoutingTool = "claude" | "codex" | "gemini" | "grokbuild" | "opencode" | "openclaw" | "hermes";
export type RoutingMember = { kind: "profile"; profileId: string } | { kind: "group"; groupId: string };
export interface RoutingGroup {
  id: string;
  name: string;
  mode: "ordered" | "roundRobin" | "manual";
  members: RoutingMember[];
  pickedProfileId: string | null;
}
export interface RoutingRule {
  id: string;
  name: string;
  groupId: string;
  model: string;
  matchMode: "exact" | "prefix" | "contains";
  images: boolean;
  thinking: boolean;
  minRequestBytes: number;
}
export interface RoutingPolicy {
  enabled: boolean;
  defaultGroupId: string | null;
  groups: RoutingGroup[];
  rules: RoutingRule[];
}
export interface RoutingDocument {
  revision: string | null;
  policy: RoutingPolicy;
}
export interface RoutingPreview {
  groupId: string | null;
  ruleId: string | null;
  profileIds: string[];
  reason: string;
}
export interface RoutingProfile {
  providerId: string;
  providerName: string;
}

export function moveRoutingItem<T>(items: readonly T[], index: number, offset: -1 | 1): T[] {
  const next = [...items];
  const target = index + offset;
  if (index < 0 || index >= items.length || target < 0 || target >= items.length) return next;
  [next[index], next[target]] = [next[target], next[index]];
  return next;
}

export function removeRoutingGroup(policy: RoutingPolicy, id: string): RoutingPolicy {
  if (
    policy.groups.some(
      (group) => group.id !== id && group.members.some((member) => member.kind === "group" && member.groupId === id),
    )
  ) {
    throw new Error("referencedGroup");
  }
  return {
    ...policy,
    groups: policy.groups.filter((group) => group.id !== id),
    rules: policy.rules.filter((rule) => rule.groupId !== id),
    defaultGroupId: policy.defaultGroupId === id ? null : policy.defaultGroupId,
  };
}
