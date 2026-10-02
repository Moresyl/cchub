import { Component, lazy, Suspense, type ReactNode } from "react";
import type { CodeEditorProps } from "./CodeEditor";
import { getLocale } from "../lib/i18n";
import { Textarea } from "./ui/textarea";

interface EditorBoundaryProps {
  children: ReactNode;
  minHeight: number;
  editorProps: CodeEditorProps;
  failedLabel: string;
}

const InitialEditor = lazy(() => import("./CodeEditor"));

class EditorBoundary extends Component<EditorBoundaryProps, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <div
        className="flex flex-col overflow-hidden rounded-md border border-border bg-[var(--bg-input)]"
        style={{
          minHeight: this.props.minHeight,
          maxHeight: this.props.editorProps.maxHeight,
          ...(this.props.editorProps.fillHeight ? { height: "100%", flex: "1 1 auto" } : {}),
        }}
      >
        <p role="alert" className="shrink-0 border-b border-border px-3 py-2 text-xs text-muted-foreground">
          {this.props.failedLabel}
        </p>
        <Textarea
          value={this.props.editorProps.value}
          onChange={(event) => this.props.editorProps.onChange?.(event.target.value)}
          readOnly={this.props.editorProps.readOnly}
          aria-label={
            this.props.editorProps.ariaLabel ??
            `${(this.props.editorProps.language ?? "json").toUpperCase()} configuration editor`
          }
          placeholder={this.props.editorProps.placeholder}
          spellCheck={false}
          className="min-h-0 flex-1 resize-none rounded-none border-0 font-mono text-xs"
          style={{ minHeight: this.props.editorProps.fillHeight ? 0 : Math.max(48, this.props.minHeight - 44) }}
        />
      </div>
    );
  }
}

// Keep loading and failure inside the field. Browsers may cache failed module
// imports, so offer a raw field instead of a retry that could discard the draft.
export default function DeferredCodeEditor(props: CodeEditorProps) {
  const locale = getLocale();
  const text = (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const minHeight = props.fillHeight ? 0 : Math.min(props.minHeight ?? 120, props.maxHeight ?? Infinity);
  return (
    <EditorBoundary
      minHeight={minHeight}
      editorProps={props}
      failedLabel={text(
        "代码高亮暂不可用，已保留原始文本。",
        "Code highlighting is unavailable. Your original text is preserved.",
        "コードの強調表示を利用できません。元のテキストは保持されています。",
      )}
    >
      <Suspense
        fallback={
          <div
            role="status"
            className="flex items-center justify-center gap-2 rounded-md border border-border bg-[var(--bg-input)] text-xs text-muted-foreground"
            style={{
              minHeight,
              maxHeight: props.maxHeight,
              ...(props.fillHeight ? { height: "100%", flex: "1 1 auto" } : {}),
            }}
          >
            <span className="spinner size-3" aria-hidden="true" />
            {text("正在加载编辑器…", "Loading editor…", "エディターを読み込み中…")}
          </div>
        }
      >
        <InitialEditor {...props} />
      </Suspense>
    </EditorBoundary>
  );
}
