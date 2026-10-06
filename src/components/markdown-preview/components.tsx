import { createContext, useContext, type ComponentProps } from "react";
import type { Components } from "react-markdown";
import { usePreferences } from "../../stores/preferences";
import { Checkbox } from "../ui/checkbox";

const TaskLabel = createContext("");
type ContentNode = { type: string; tagName?: string; value?: string; children?: ContentNode[] };

function taskText(node: ContentNode | undefined): string {
  if (!node) return "";
  if (node.type === "text") return node.value ?? "";
  if (node.tagName && ["input", "ul", "ol"].includes(node.tagName)) return "";
  return node.children?.map(taskText).join("") ?? "";
}

function MarkdownCheckbox({ checked, type }: ComponentProps<"input">) {
  const label = useContext(TaskLabel);
  const locale = usePreferences((state) => state.locale);
  if (type !== "checkbox") return null;
  return (
    <Checkbox
      checked={!!checked}
      disabled
      aria-label={label || (locale === "zh" ? "待办项" : locale === "ja" ? "タスク" : "Task")}
    />
  );
}

function MarkdownTable({ children }: ComponentProps<"table">) {
  const locale = usePreferences((state) => state.locale);
  return (
    <div
      className="markdown-table-scroll"
      role="region"
      tabIndex={0}
      aria-label={
        locale === "zh" ? "表格，可横向滚动" : locale === "ja" ? "表、横方向にスクロール" : "Table, scroll horizontally"
      }
    >
      <table>{children}</table>
    </div>
  );
}

export const previewComponents: Components = {
  input: MarkdownCheckbox,
  table: MarkdownTable,
  li: ({ children, className, node }) => (
    <li className={className}>
      <TaskLabel.Provider value={taskText(node).trim()}>{children}</TaskLabel.Provider>
    </li>
  ),
};
