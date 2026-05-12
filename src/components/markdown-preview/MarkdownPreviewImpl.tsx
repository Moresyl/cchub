import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface MarkdownPreviewImplProps {
  content: string;
}

export default function MarkdownPreviewImpl({ content }: MarkdownPreviewImplProps) {
  return <Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown>;
}
