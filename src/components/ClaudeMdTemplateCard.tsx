import { memo } from "react";

interface ClaudeMdTemplateCardItem {
  id: string;
  name: string;
  description: string;
  file_name: string;
  content: string;
  tool_name: string;
}

interface ClaudeMdTemplateCardProps {
  template: ClaudeMdTemplateCardItem;
  onCreate: (template: ClaudeMdTemplateCardItem) => void | Promise<void>;
}

function ClaudeMdTemplateCardComponent({
  template,
  onCreate,
}: ClaudeMdTemplateCardProps) {
  return (
    <div
      className="card card-interactive"
      style={{ padding: "14px 18px" }}
      onClick={() => onCreate(template)}
    >
      <div style={{ fontSize: 13, fontWeight: 600 }}>{template.name}</div>
      <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>{template.file_name}</div>
      <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>{template.description}</div>
    </div>
  );
}

export default memo(ClaudeMdTemplateCardComponent);
