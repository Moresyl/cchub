import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import MasterDetailLayout from "./MasterDetailLayout";

describe("MasterDetailLayout", () => {
  it("keeps the list visible and exposes detail as a labelled complementary region", () => {
    render(<MasterDetailLayout list={<div>Items</div>} detail={<div>Details</div>} detailLabel="Server details" />);

    expect(screen.getByText("Items")).toBeTruthy();
    expect(screen.getByRole("complementary", { name: "Server details" }).textContent).toContain("Details");
  });

  it("does not reserve an empty detail pane", () => {
    render(<MasterDetailLayout list={<div>Items</div>} detailLabel="Server details" />);
    expect(screen.queryByRole("complementary")).toBeNull();
  });
});
