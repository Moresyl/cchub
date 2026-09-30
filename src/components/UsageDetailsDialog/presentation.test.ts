import { describe, expect, it } from "vitest";
import { number, resetTime, usagePercent, usageRows } from "./presentation";

describe("usage presentation", () => {
  it("rejects non-finite and non-numeric values while preserving zero", () => {
    for (const value of [null, undefined, {}, true, "", " ", "NaN", "inf", Infinity]) expect(number(value)).toBeNull();
    expect(number(" 0 ")).toBe(0);
    expect(number(-1)).toBe(-1);
  });
  it("does not invent percentages from incomplete or overflowed metrics", () => {
    for (const row of [
      { total: 100 },
      { remaining: 5 },
      { used: 0, total: 0 },
      { used: 1e300, total: 1e-300 },
      { utilization: "NaN" },
    ]) {
      expect(usagePercent(row)).toBeNull();
    }
    expect(usagePercent({ remaining: 25, total: 100 })).toBe(75);
    expect(usagePercent({ remaining: -1, total: 10 })).toBe(100);
    expect(usagePercent({ used: 0, total: 100 })).toBe(0);
  });
  it("accepts object rows and explicitly dated resets only", () => {
    expect(usageRows([null, [1], { balance: 2 }, "text"])).toEqual([{ balance: 2 }]);
    expect(resetTime("15", "zh")).toBeNull();
    expect(resetTime("nonsense", "zh")).toBeNull();
    expect(resetTime("2027-01-15T08:00:00Z", "zh")).not.toBeNull();
  });
});
