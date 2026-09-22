import { describe, expect, it } from "vitest";
import { getNavigationSection, navigationSections } from "./navigation";
import { pageImports } from "./routes";

describe("navigation metadata", () => {
  it("covers every route exactly once", () => {
    const navigationPaths = navigationSections.flatMap((section) => section.items.map((item) => item.path));

    expect(new Set(navigationPaths).size).toBe(navigationPaths.length);
    expect([...navigationPaths].sort()).toEqual(Object.keys(pageImports).sort());
  });

  it("resolves a contextual section and safely falls back to overview", () => {
    expect(getNavigationSection("/skills").key).toBe("ecosystem");
    expect(getNavigationSection("/usage").key).toBe("operations");
    expect(getNavigationSection("/not-found").key).toBe("overview");
    expect(getNavigationSection("/").items[0].labelKey).toBe("profiles");
  });

  it("uses a route inside each section as its default destination", () => {
    for (const section of navigationSections) {
      expect(section.items.some((item) => item.path === section.defaultPath)).toBe(true);
    }
  });
});
