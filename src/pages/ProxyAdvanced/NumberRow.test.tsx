import { useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import NumberRow from "./NumberRow";

afterEach(cleanup);

function setup(value = 600, min = 0, max = 86400) {
  const changed = vi.fn();
  function Harness() {
    const [current, setCurrent] = useState(value);
    return (
      <NumberRow
        label="Timeout"
        description="Seconds; zero disables"
        value={current}
        min={min}
        max={max}
        onChange={(next) => {
          changed(next);
          setCurrent(next);
        }}
      />
    );
  }
  render(<Harness />);
  return { input: screen.getByRole("spinbutton", { name: "Timeout" }) as HTMLInputElement, changed };
}

describe("NumberRow editing", () => {
  it("allows clearing and retyping without coercing an empty field into zero", () => {
    const { input, changed } = setup();
    fireEvent.change(input, { target: { value: "" } });
    expect(input.value).toBe("");
    expect(changed).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "12" } });
    expect(input.value).toBe("12");
    expect(changed).toHaveBeenLastCalledWith(12);
  });

  it("saves an explicit zero and restores a blank field on blur", () => {
    const { input, changed } = setup();
    fireEvent.change(input, { target: { value: "0" } });
    expect(changed).toHaveBeenLastCalledWith(0);
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.blur(input);
    expect(input.value).toBe("0");
    expect(changed).toHaveBeenCalledTimes(1);
  });

  it("keeps out-of-range drafts while typing and clamps only on commit", () => {
    const { input, changed } = setup(3, 1, 20);
    fireEvent.change(input, { target: { value: "200" } });
    expect(input.value).toBe("200");
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(changed).not.toHaveBeenCalled();
    fireEvent.blur(input);
    expect(input.value).toBe("20");
    expect(changed).toHaveBeenCalledWith(20);
  });

  it("rejects fractional seconds and restores the last valid value with Escape", () => {
    const { input, changed } = setup(60);
    fireEvent.change(input, { target: { value: "2.5" } });
    fireEvent.blur(input);
    expect(input.value).toBe("60");
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(input.value).toBe("60");
    expect(changed).not.toHaveBeenCalled();
  });

  it("connects its description and refreshes when configuration is reloaded", () => {
    const changed = vi.fn();
    const { rerender } = render(<NumberRow label="Timeout" description="Help" value={60} onChange={changed} />);
    const input = screen.getByRole("spinbutton", { name: "Timeout" }) as HTMLInputElement;
    expect(document.getElementById(input.getAttribute("aria-describedby")!)?.textContent).toBe("Help");
    rerender(<NumberRow label="Timeout" description="Help" value={180} onChange={changed} />);
    expect(input.value).toBe("180");
  });
});
