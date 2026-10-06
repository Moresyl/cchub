import Markdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { previewComponents } from "./components";

interface MarkdownPreviewImplProps {
  content: string;
  components?: Components;
}

export default function MarkdownPreviewImpl({ content, components }: MarkdownPreviewImplProps) {
  return (
    <div className="markdown-content">
      <Markdown remarkPlugins={[remarkGfm]} components={{ ...previewComponents, ...components }}>
        {content}
      </Markdown>
    </div>
  );
}
