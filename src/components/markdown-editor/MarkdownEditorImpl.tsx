import { useCallback, useRef } from "react";
import {
  MDXEditor,
  headingsPlugin,
  listsPlugin,
  quotePlugin,
  thematicBreakPlugin,
  linkPlugin,
  linkDialogPlugin,
  tablePlugin,
  codeBlockPlugin,
  codeMirrorPlugin,
  markdownShortcutPlugin,
  toolbarPlugin,
  BoldItalicUnderlineToggles,
  BlockTypeSelect,
  ListsToggle,
  CreateLink,
  InsertTable,
  InsertThematicBreak,
  InsertCodeBlock,
  type MDXEditorMethods,
} from "@mdxeditor/editor";
import "@mdxeditor/editor/style.css";
import { getTheme } from "../../lib/theme";

interface MarkdownEditorImplProps {
  value: string;
  onChange: (value: string) => void;
  minHeight?: number;
}

const CODE_BLOCK_LANGUAGES = {
  "": "Plain",
  js: "JavaScript",
  ts: "TypeScript",
  py: "Python",
  rust: "Rust",
  bash: "Bash",
  json: "JSON",
};

export default function MarkdownEditorImpl({
  value,
  onChange,
  minHeight = 400,
}: MarkdownEditorImplProps) {
  const editorRef = useRef<MDXEditorMethods>(null);
  const isDark = getTheme() === "dark";

  const handleChange = useCallback(
    (markdown: string) => {
      onChange(markdown);
    },
    [onChange],
  );

  return (
    <div
      className="mdx-editor-wrapper"
      style={{
        borderRadius: 6,
        border: "1px solid var(--border-default)",
        overflow: "hidden",
        minHeight,
      }}
    >
      <MDXEditor
        ref={editorRef}
        markdown={value}
        onChange={handleChange}
        className={isDark ? "dark-theme" : ""}
        contentEditableClassName="mdx-editor-content"
        plugins={[
          headingsPlugin(),
          listsPlugin(),
          quotePlugin(),
          thematicBreakPlugin(),
          linkPlugin(),
          linkDialogPlugin(),
          tablePlugin(),
          codeBlockPlugin({ defaultCodeBlockLanguage: "" }),
          codeMirrorPlugin({
            codeBlockLanguages: CODE_BLOCK_LANGUAGES,
            autoLoadLanguageSupport: false,
          }),
          markdownShortcutPlugin(),
          toolbarPlugin({
            toolbarContents: () => (
              <>
                <BoldItalicUnderlineToggles />
                <BlockTypeSelect />
                <ListsToggle />
                <CreateLink />
                <InsertTable />
                <InsertCodeBlock />
                <InsertThematicBreak />
              </>
            ),
          }),
        ]}
      />
    </div>
  );
}
