import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import SessionEntries from "./Entries";

describe("SessionEntries", () => {
  it("shows the empty state", () => {
    render(<SessionEntries entries={[]} query="" emptyLabel="No matches" />);
    expect(screen.getByText("No matches")).toBeTruthy();
  });

  it("renders record details and keeps the complete title available", () => {
    const { container } = render(
      <SessionEntries
        entries={[{ id: "one", kind: "user", title: "A long record title", content: "record body", timestamp: null }]}
        query="record"
        emptyLabel="No matches"
      />,
    );
    expect(screen.getByTitle("A long record title")).toBeTruthy();
    expect(container.querySelector("pre")?.textContent).toBe("record body");
  });
});
