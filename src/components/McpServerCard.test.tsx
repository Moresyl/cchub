import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import McpServerCard from "./McpServerCard";

afterEach(cleanup);
it.each(["stdio", "http", "sse"])("offers shared edit actions for %s without selecting the card", (transport) => {
  const server = {
    id: "one",
    name: "Fixture",
    command: transport === "stdio" ? "node" : "https://example.test/mcp",
    args: '["server.js"]',
    env: "{}",
    status: "active",
    transport,
    source: "local",
    package_name: null,
    version: null,
    config_path: null,
  };
  const onSelect = vi.fn();
  const onEdit = vi.fn();
  render(
    <McpServerCard
      server={server}
      selected={false}
      sourceBadge="badge-muted"
      sourceLabel="Local"
      healthStatus={null}
      healthTitle={null}
      editTitle="Edit"
      deleteTitle="Delete"
      onSelect={onSelect}
      onEdit={onEdit}
      onDelete={vi.fn()}
    />,
  );
  const edit = screen.getByRole("button", { name: "Edit" });
  expect(edit.getAttribute("data-slot")).toBe("button");
  fireEvent.click(edit);
  expect(onEdit).toHaveBeenCalledWith(server);
  expect(onSelect).not.toHaveBeenCalled();
  expect(screen.getByText(transport === "stdio" ? "node server.js" : "https://example.test/mcp")).toBeTruthy();
});
