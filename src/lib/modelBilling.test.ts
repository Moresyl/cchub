import { describe, expect, it } from "vitest";
import { agreeModelBilling, normalizeModelBilling } from "./modelBilling";
import { modelBillingLabel } from "../components/ModelBillingBadge";

describe("premium-request billing", () => {
  it("preserves zero, fractional and uncertain premium units", () => {
    for (const billing of [
      { kind: "free", multiplier: 0 },
      { kind: "premium", multiplier: 0.33 },
      { kind: "premium", multiplier: null },
    ]) {
      expect(normalizeModelBilling(billing)).toEqual(billing);
    }
  });
  it("keeps malformed, negative, infinite and contradictory units unknown", () => {
    for (const billing of [
      null,
      {},
      "free",
      { kind: "free", multiplier: 1 },
      { kind: "premium", multiplier: 0 },
      { kind: "premium", multiplier: -1 },
      { kind: "premium", multiplier: Infinity },
      { kind: "premium", multiplier: NaN },
      { kind: "premium", multiplier: "0.33" },
      { kind: "premium" },
      { kind: "unknown", multiplier: 0 },
    ]) {
      expect(normalizeModelBilling(billing)).toEqual({ kind: "unknown", multiplier: null });
    }
  });
  it("merges conflicting rates without losing known premium classification", () => {
    expect(agreeModelBilling({ kind: "premium", multiplier: 0.33 }, { kind: "premium", multiplier: 0.33 })).toEqual({
      kind: "premium",
      multiplier: 0.33,
    });
    expect(agreeModelBilling({ kind: "premium", multiplier: 1 }, { kind: "premium", multiplier: 2 })).toEqual({
      kind: "premium",
      multiplier: null,
    });
    expect(agreeModelBilling({ kind: "premium", multiplier: 1 }, { kind: "free", multiplier: 0 })).toEqual({
      kind: "unknown",
      multiplier: null,
    });
  });
  it("labels request units without claiming a monetary token price", () => {
    const text = (zh: string) => zh;
    expect(modelBillingLabel({ kind: "free", multiplier: 0 }, text)).toBe("不消耗高级额度");
    expect(modelBillingLabel({ kind: "premium", multiplier: 0.33 }, text)).toBe("高级请求 ×0.33");
    expect(modelBillingLabel({ kind: "premium", multiplier: null }, text)).toBe("高级请求 · 倍率未确定");
    expect(modelBillingLabel(null, text)).toBe("计费信息未提供");
  });
});
