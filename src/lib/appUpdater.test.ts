import { describe, expect, it } from "vitest";
import { compareVersions, normalizeVersion } from "./appUpdater";

describe("app updater version handling", () => {
  it("normalizes release tags", () => {
    expect(normalizeVersion("v1.4.6")).toBe("1.4.6");
    expect(normalizeVersion("release-2.0.0")).toBe("2.0.0");
  });

  it("compares numeric segments instead of lexical strings", () => {
    expect(compareVersions("1.10.0", "1.9.9")).toBeGreaterThan(0);
    expect(compareVersions("1.4", "1.4.0")).toBe(0);
    expect(compareVersions("1.3.9", "1.4.0")).toBeLessThan(0);
  });

  it("keeps stable releases newer than prereleases", () => {
    expect(compareVersions("1.4.6", "1.4.6-beta.2")).toBeGreaterThan(0);
    expect(compareVersions("1.4.6-beta.10", "1.4.6-beta.2")).toBeGreaterThan(0);
    expect(compareVersions("1.4.6-beta.2", "1.4.6")).toBeLessThan(0);
  });
});
