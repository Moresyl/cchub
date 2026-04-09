import { lazy, memo, Suspense } from "react";

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  minHeight?: number;
}

interface MarkdownEditorFallbackProps {
  minHeight: number;
}

const MarkdownEditorImpl = lazy(() => import("./markdown-editor/MarkdownEditorImpl"));

function MarkdownEditorFallbackComponent({ minHeight }: MarkdownEditorFallbackProps) {
  return (
    <div
      className="mdx-editor-wrapper"
      style={{
        borderRadius: 6,
        border: "1px solid var(--border-default)",
        overflow: "hidden",
        minHeight,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div className="spinner" />
    </div>
  );
}

const MarkdownEditorFallback = memo(MarkdownEditorFallbackComponent);

function MarkdownEditorComponent({ value, onChange, minHeight = 400 }: MarkdownEditorProps) {
  return (
    <Suspense fallback={<MarkdownEditorFallback minHeight={minHeight} />}>
      <MarkdownEditorImpl value={value} onChange={onChange} minHeight={minHeight} />
    </Suspense>
  );
}

export default memo(MarkdownEditorComponent);
