import { describe, expect, it } from "vitest";
import { moveRoutingItem, removeRoutingGroup, type RoutingPolicy } from "./providerRouting";

describe("routing drafts", () => {
  it("retains the original order on boundary moves without mutating the original", () => {
    const values = Object.freeze(["a", "b", "c"]);
    expect(moveRoutingItem(values, 0, -1)).toEqual(values);
    expect(moveRoutingItem(values, 2, 1)).toEqual(values);
    expect(moveRoutingItem(values, 99, -1)).toEqual(values);
    expect(moveRoutingItem(values, 1, -1)).toEqual(["b", "a", "c"]);
    expect(values).toEqual(["a", "b", "c"]);
  });
  it("protects a referenced child group and removes only owned rules when unreferenced", () => {
    const policy: RoutingPolicy = {
      enabled: true,
      defaultGroupId: "a",
      groups: [
        { id: "a", name: "A", mode: "ordered", members: [{ kind: "profile", profileId: "p1" }], pickedProfileId: null },
        { id: "b", name: "B", mode: "ordered", members: [{ kind: "group", groupId: "a" }], pickedProfileId: null },
      ],
      rules: [
        {
          id: "r",
          name: "Rule",
          groupId: "a",
          model: "m",
          matchMode: "exact",
          images: false,
          thinking: false,
          minRequestBytes: 0,
        },
      ],
    };
    expect(() => removeRoutingGroup(policy, "a")).toThrow("referencedGroup");
    expect(policy.groups).toHaveLength(2);
    const result = removeRoutingGroup(removeRoutingGroup(policy, "b"), "a");
    expect(result.groups).toEqual([]);
    expect(result.rules).toEqual([]);
    expect(result.defaultGroupId).toBeNull();
    expect(policy.rules).toHaveLength(1);
  });
});
