import { lazy, memo, Suspense } from "react";

interface MarkdownPreviewProps {
  content: string;
  loadingLabel?: string;
}

// MarkdownPreviewImpl 把 react-markdown + remark-gfm 全部锁在内，仅在
// preview dialog 真正挂载时才会拉取 markdown-rendering chunk（约 221KB）。
const MarkdownPreviewImpl = lazy(() => import("./markdown-preview/MarkdownPreviewImpl"));

function MarkdownPreviewComponent({ content, loadingLabel }: MarkdownPreviewProps) {
  return (
    <Suspense
      fallback={
        loadingLabel ? (
          <p role="status" className="text-xs text-muted-foreground">
            {loadingLabel}
          </p>
        ) : null
      }
    >
      <MarkdownPreviewImpl content={content} />
    </Suspense>
  );
}

export default memo(MarkdownPreviewComponent);
