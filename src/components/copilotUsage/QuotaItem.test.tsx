import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { QuotaItem } from "./QuotaItem";
import type { CopilotQuota } from "../../lib/copilotAccounts";

afterEach(cleanup);
const labels = { label: "Premium", unknownLabel: "未提供", unlimitedLabel: "不限量" };
const quota: CopilotQuota = { entitlement: 100, remaining: 49.5, percent_remaining: null, unlimited: false };
describe("reported quota display", () => {
  it("derives a bounded percentage from fractional reported counts", () => {
    render(<QuotaItem {...labels} value={quota} />);
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("49.5");
    expect(screen.getByText("49.5 / 100")).toBeTruthy();
  });
  it.each([
    null,
    { ...quota, entitlement: null, remaining: null },
    { ...quota, entitlement: NaN, remaining: Infinity, percent_remaining: NaN },
  ])("keeps missing or invalid quota unknown", (value) => {
    render(<QuotaItem {...labels} value={value} />);
    expect(screen.getByText("未提供")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });
  it("shows unlimited allowance without inventing finite counts", () => {
    render(<QuotaItem {...labels} value={{ ...quota, entitlement: null, remaining: null, unlimited: true }} />);
    expect(screen.getByText("不限量")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });
  it.each([-20, 150])("clamps a reported percentage %s to the visible range", (percent) => {
    render(<QuotaItem {...labels} value={{ ...quota, percent_remaining: percent }} />);
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe(
      String(Math.max(0, Math.min(100, percent))),
    );
  });
});
