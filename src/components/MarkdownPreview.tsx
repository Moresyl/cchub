import { lazy, memo, Suspense } from "react";
import type { Components } from "react-markdown";

interface MarkdownPreviewProps {
  content: string;
  loadingLabel?: string;
  components?: Components;
}

// MarkdownPreviewImpl 把 react-markdown + remark-gfm 全部锁在内，仅在
// preview dialog 真正挂载时才会拉取 markdown-rendering chunk（约 221KB）。
const MarkdownPreviewImpl = lazy(() => import("./markdown-preview/MarkdownPreviewImpl"));

function MarkdownPreviewComponent({ content, loadingLabel, components }: MarkdownPreviewProps) {
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
      <MarkdownPreviewImpl content={content} components={components} />
    </Suspense>
  );
}

export default memo(MarkdownPreviewComponent);
