import { describe, expect, it } from "vitest";
import { getRequestTiming, requestTimingLabel } from "./requestTiming";

const record = {
  is_streaming: true,
  status_code: 200,
  latency_ms: 2500,
  output_tokens: 60,
  first_output_ms: 500,
  generation_ms: 1500,
};

describe("observed stream timing", () => {
  it("uses observed output duration without counting trailing completion wait", () => {
    expect(getRequestTiming(record)).toEqual({ firstOutputMs: 500, tokensPerSecond: 40 });
    expect(requestTimingLabel(record, (zh) => zh)).toBe("首个输出 500 ms · ~40.0 tok/s");
  });

  it("does not fabricate timings for historical, nonstreaming or invalid records", () => {
    for (const fields of [
      { first_output_ms: undefined },
      { first_output_ms: null },
      { first_output_ms: -1 },
      { first_output_ms: NaN },
      { first_output_ms: Infinity },
      { first_output_ms: 2600 },
      { first_output_ms: 0.5 },
      { latency_ms: NaN },
      { is_streaming: false },
    ]) {
      const value = { ...record, ...fields };
      expect(getRequestTiming(value)).toEqual({ firstOutputMs: null, tokensPerSecond: null });
      expect(requestTimingLabel(value, (_, en) => en)).toBeUndefined();
    }
    expect(getRequestTiming({ ...record, first_output_ms: 0 }).firstOutputMs).toBe(0);
  });

  it("hides short bursts, unknown durations, invalid counters and incomplete outcomes", () => {
    for (const fields of [
      { generation_ms: undefined },
      { generation_ms: null },
      { generation_ms: 0 },
      { generation_ms: 99 },
      { generation_ms: 2100 },
      { generation_ms: Infinity },
      { generation_ms: -1 },
      { output_tokens: 0 },
      { output_tokens: NaN },
      { output_tokens: -1 },
      { status_code: 499 },
      { status_code: 502 },
    ]) {
      expect(getRequestTiming({ ...record, ...fields })).toEqual({ firstOutputMs: 500, tokensPerSecond: null });
    }
    expect(getRequestTiming({ ...record, generation_ms: 100 }).tokensPerSecond).toBe(600);
    expect(requestTimingLabel({ ...record, generation_ms: 0 }, (_, en) => en)).toBe("First output 500 ms");
  });
});
