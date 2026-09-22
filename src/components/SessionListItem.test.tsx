import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import SessionListItem, { type SessionListItemSession } from "./SessionListItem";

const readonlySession: SessionListItemSession = {
  id: "readonly-session",
  tool_id: "mcode",
  tool_name: "MiniMax",
  title: "Read-only session",
  cwd: null,
  source_kind: "sqlite",
  source_backend: "mcode_sqlite",
  source_path: "runtime-state.sqlite",
  created_at: null,
  updated_at: null,
  preview: "Stored in MiniMax runtime state",
  message_count: 2,
  input_tokens: null,
  output_tokens: null,
  tokens_used: null,
  search_hit_count: 0,
  can_resume: true,
  can_delete: false,
};

describe("SessionListItem", () => {
  it("disables selection and deletion for read-only sessions", () => {
    const onToggleChecked = vi.fn();
    const onDelete = vi.fn();

    render(
      <SessionListItem
        session={readonlySession}
        selected={false}
        query=""
        resumeCommand="mcode --session readonly-session"
        deleting={false}
        checked={false}
        copyLabel="Copy"
        copyTitle="Copy resume command"
        deleteTitle="Delete session"
        deleteLabel="Delete"
        selectLabel="Select session"
        tokenLabel={(count) => `${count} tokens`}
        unknownTimeLabel="Unknown"
        matchLabel={(count) => `${count} matches`}
        itemsLabel={(count) => `${count} messages`}
        onOpen={vi.fn()}
        onToggleChecked={onToggleChecked}
        onCopyResume={vi.fn()}
        onDelete={onDelete}
      />,
    );

    const selectButton = screen.getByTitle("Select session");
    const deleteButton = screen.getByTitle("Delete session");
    expect((selectButton as HTMLButtonElement).disabled).toBe(true);
    expect((deleteButton as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(selectButton);
    fireEvent.click(deleteButton);
    expect(onToggleChecked).not.toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();
  });
});
