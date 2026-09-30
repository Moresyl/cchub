import { describe, expect, it } from "vitest";
import { defaultSettings, validateSettings } from "./types";

describe("usage alert thresholds", () => {
  it("accepts zero balances and separate units, but rejects incomplete, duplicate or nonfinite values", () => {
    expect(validateSettings(defaultSettings)).toBe(true);
    expect(validateSettings({ ...defaultSettings, enabled: true })).toBe(false);
    expect(validateSettings({ ...defaultSettings, enabled: true, quotaPercent: 80 })).toBe(true);
    for (const quotaPercent of [0, 101, NaN, Infinity])
      expect(validateSettings({ ...defaultSettings, quotaPercent })).toBe(false);
    expect(
      validateSettings({
        ...defaultSettings,
        enabled: true,
        balances: [
          { unit: "USD", amount: 0 },
          { unit: "CNY", amount: 5 },
        ],
      }),
    ).toBe(true);
    expect(
      validateSettings({
        ...defaultSettings,
        balances: [
          { unit: "USD", amount: 5 },
          { unit: " usd ", amount: 5 },
        ],
      }),
    ).toBe(false);
    for (const unit of ["", "U\nSD", "界".repeat(11)])
      expect(validateSettings({ ...defaultSettings, balances: [{ unit, amount: 5 }] })).toBe(false);
    for (const amount of [-1, NaN, Infinity])
      expect(validateSettings({ ...defaultSettings, balances: [{ unit: "USD", amount }] })).toBe(false);
  });
});
