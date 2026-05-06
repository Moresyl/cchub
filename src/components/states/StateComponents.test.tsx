import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import EmptyState from "./EmptyState";
import ErrorState from "./ErrorState";
import LoadingState from "./LoadingState";

describe("state components", () => {
  it("renders loading state with optional label", () => {
    render(<LoadingState label="Loading profiles" />);

    expect(screen.getByText("Loading profiles")).toBeTruthy();
  });

  it("renders empty state title, description, and action", () => {
    render(
      <EmptyState
        title="No profiles"
        description="Create a profile to get started."
        action={<button type="button">Create</button>}
      />,
    );

    expect(screen.getByText("No profiles")).toBeTruthy();
    expect(screen.getByText("Create a profile to get started.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create" })).toBeTruthy();
  });

  it("renders retryable error state", () => {
    const onRetry = vi.fn();
    render(<ErrorState title="Load failed" message="Network unavailable" retryLabel="Try again" onRetry={onRetry} />);

    screen.getByRole("button", { name: "Try again" }).click();

    expect(screen.getByRole("alert")).toBeTruthy();
    expect(screen.getByText("Load failed")).toBeTruthy();
    expect(screen.getByText("Network unavailable")).toBeTruthy();
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});
