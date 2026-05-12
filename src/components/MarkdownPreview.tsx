import { lazy, memo, Suspense } from "react";

interface MarkdownPreviewProps {
  content: string;
}

// MarkdownPreviewImpl 把 react-markdown + remark-gfm 全部锁在内，仅在
// preview dialog 真正挂载时才会拉取 markdown-rendering chunk（约 221KB）。
const MarkdownPreviewImpl = lazy(() => import("./markdown-preview/MarkdownPreviewImpl"));

function MarkdownPreviewComponent({ content }: MarkdownPreviewProps) {
  return (
    <Suspense fallback={null}>
      <MarkdownPreviewImpl content={content} />
    </Suspense>
  );
}

export default memo(MarkdownPreviewComponent);
