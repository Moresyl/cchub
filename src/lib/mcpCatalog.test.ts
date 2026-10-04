import { expect, it } from "vitest";
import { knownMcpStates, readMcpStatuses } from "./mcpCatalog";

it("keeps conflicts and independent entries out of writable boolean states", () => {
  const states = readMcpStatuses(
    {
      one: {
        claude: { state: "source", disabled: true },
        codex: { state: "linked", disabled: false },
        gemini: { state: "unowned", disabled: false },
        hermes: { state: "conflict", disabled: false },
        mcode: { state: "missing", disabled: false },
      },
    },
    ["one"],
  );
  expect(knownMcpStates(states.one)).toEqual({ claude: true, codex: true, mcode: false });
});

it.each([
  null,
  [],
  {},
  { one: null },
  { one: { claude: false } },
  { one: { claude: { state: "missing", disabled: "false" } } },
])("rejects malformed or missing status data: %j", (value) => {
  expect(() => readMcpStatuses(value, ["one"])).toThrow();
});
