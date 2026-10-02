import Markdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

interface MarkdownPreviewImplProps {
  content: string;
  components?: Components;
}

export default function MarkdownPreviewImpl({ content, components }: MarkdownPreviewImplProps) {
  return (
    <Markdown remarkPlugins={[remarkGfm]} components={components}>
      {content}
    </Markdown>
  );
}
