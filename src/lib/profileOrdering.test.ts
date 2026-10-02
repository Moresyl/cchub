import { describe, expect, it } from "vitest";
import { profileMoveTarget, reorderedProfileIds, type ProfileMoveDirection } from "./profileOrdering";

describe("profile ordering", () => {
  it.each<[number, number, ProfileMoveDirection, number | null]>([
    [1, 4, "up", 0],
    [1, 4, "down", 2],
    [2, 4, "first", 0],
    [1, 4, "last", 3],
    [0, 4, "up", null],
    [0, 4, "first", null],
    [3, 4, "down", null],
    [3, 4, "last", null],
    [-1, 4, "down", null],
    [4, 4, "first", null],
    [0, 0, "last", null],
    [0, 1, "down", null],
  ])("moves index %i in %i items %s to %s", (index, count, direction, expected) => {
    expect(profileMoveTarget(index, count, direction)).toBe(expected);
  });

  it("uses displayed order in both directions without mutating its input", () => {
    const ids = Object.freeze(["c", "a", "b", "d"]);
    expect(reorderedProfileIds(ids, "c", "b")).toEqual(["a", "b", "c", "d"]);
    expect(reorderedProfileIds(ids, "d", "a")).toEqual(["c", "d", "a", "b"]);
    expect(ids).toEqual(["c", "a", "b", "d"]);
  });

  it("rejects missing IDs, duplicate IDs and moves without a change", () => {
    expect(reorderedProfileIds(["a", "b"], "x", "b")).toBeNull();
    expect(reorderedProfileIds(["a", "b"], "a", "x")).toBeNull();
    expect(reorderedProfileIds(["a", "b"], "a", "a")).toBeNull();
    expect(reorderedProfileIds(["a", "a", "b"], "a", "b")).toBeNull();
    expect(reorderedProfileIds([], "a", "b")).toBeNull();
  });
});
