import { memo, useEffect, useMemo, useRef } from "react";
import { EditorView, basicSetup } from "codemirror";
/* eslint-disable @typescript-eslint/no-explicit-any */
import { Compartment, EditorState } from "@codemirror/state";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { yaml } from "@codemirror/lang-yaml";
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import { linter, type Diagnostic } from "@codemirror/lint";
import { EditorView as CodeMirrorView, ViewUpdate, placeholder as editorPlaceholder } from "@codemirror/view";
import { tags } from "@lezer/highlight";

interface CodeEditorProps {
  value: string;
  onChange?: (value: string) => void;
  language?: "json" | "markdown" | "yaml" | "toml" | "text";
  readOnly?: boolean;
  minHeight?: number;
  maxHeight?: number;
  placeholder?: string;
}

const jsonLinter = linter((view) => {
  const diagnostics: Diagnostic[] = [];
  const text = view.state.doc.toString();
  if (!text.trim()) return diagnostics;
  try {
    JSON.parse(text);
  } catch (e: any) {
    const match = e.message?.match(/position (\d+)/);
    const pos = match ? Math.min(parseInt(match[1]), text.length) : 0;
    diagnostics.push({
      from: pos,
      to: Math.min(pos + 1, text.length),
      severity: "error",
      message: e.message || "Invalid JSON",
    });
  }
  return diagnostics;
});

const cmTheme = EditorView.theme({
  "&": {
    fontSize: "12.5px",
    fontFamily: "var(--font-code)",
    flex: "1 1 auto",
    minHeight: "0",
    height: "100%",
  },
  ".cm-scroller": {
    overflow: "auto",
    lineHeight: "1.65",
  },
  ".cm-content": {
    padding: "10px 0 18px",
    caretColor: "var(--text-primary)",
  },
  ".cm-line": {
    padding: "0 14px 0 8px",
  },
  ".cm-gutters": {
    background: "color-mix(in srgb, var(--bg-elevated) 74%, var(--bg-input))",
    borderRight: "1px solid var(--border-subtle)",
    color: "var(--text-muted)",
    fontSize: "11px",
    minWidth: "42px",
  },
  ".cm-lineNumbers .cm-gutterElement": {
    minWidth: "32px",
    padding: "0 8px 0 4px",
  },
  ".cm-activeLine": {
    background: "var(--bg-surface)",
  },
  ".cm-activeLineGutter": {
    background: "var(--bg-card-hover)",
    color: "var(--text-secondary)",
  },
  "&.cm-focused .cm-cursor": {
    borderLeftColor: "var(--text-primary)",
    borderLeftWidth: "2px",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
    background: "var(--accent-subtle) !important",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-tooltip": {
    background: "var(--bg-card)",
    border: "1px solid var(--border-default)",
    borderRadius: "6px",
  },
  ".cm-tooltip-lint": {
    padding: "4px 8px",
    fontSize: "12px",
  },
});

const darkHighlightStyle = HighlightStyle.define([
  { tag: [tags.propertyName, tags.attributeName], color: "#e89298" },
  { tag: [tags.string, tags.special(tags.string)], color: "#9dcc8c" },
  { tag: [tags.number, tags.bool, tags.null], color: "#e7b979" },
  { tag: [tags.keyword, tags.atom], color: "#c9a0dc" },
  { tag: [tags.typeName, tags.className], color: "#72c7d2" },
  { tag: [tags.variableName, tags.name], color: "#d8dee9" },
  { tag: [tags.comment, tags.meta], color: "#77808f", fontStyle: "italic" },
  { tag: tags.invalid, color: "#ff7373", textDecoration: "underline wavy" },
]);

const lightHighlightStyle = HighlightStyle.define([
  { tag: [tags.propertyName, tags.attributeName], color: "#b4232a" },
  { tag: [tags.string, tags.special(tags.string)], color: "#2f7d32" },
  { tag: [tags.number, tags.bool, tags.null], color: "#9b5b13" },
  { tag: [tags.keyword, tags.atom], color: "#7d3194" },
  { tag: [tags.typeName, tags.className], color: "#146f7a" },
  { tag: [tags.variableName, tags.name], color: "#262a31" },
  { tag: [tags.comment, tags.meta], color: "#7b818b", fontStyle: "italic" },
  { tag: tags.invalid, color: "#c92d2d", textDecoration: "underline wavy" },
]);

const themeCompartment = new Compartment();

export function getEditorCspNonce() {
  return document.querySelector<HTMLStyleElement>("style[nonce]")?.nonce ?? "";
}

function getThemeExtensions() {
  const isLight = document.documentElement.getAttribute("data-theme") === "light";
  return [cmTheme, syntaxHighlighting(isLight ? lightHighlightStyle : darkHighlightStyle)];
}

function getLangExtension(language: string) {
  switch (language) {
    case "json":
      return [json(), jsonLinter];
    case "yaml":
      return [yaml()];
    case "toml":
      return [StreamLanguage.define(toml)];
    case "markdown":
      return [markdown()];
    case "text":
      return [];
    default:
      return [];
  }
}

function CodeEditorComponent({
  value,
  onChange,
  language = "json",
  readOnly = false,
  minHeight = 120,
  maxHeight,
  placeholder,
}: CodeEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  const extensions = useMemo(() => {
    const nextExtensions = [
      basicSetup,
      EditorView.cspNonce.of(getEditorCspNonce()),
      ...getLangExtension(language),
      themeCompartment.of(getThemeExtensions()),
      EditorView.lineWrapping,
      CodeMirrorView.contentAttributes.of({
        "aria-label": `${language.toUpperCase()} configuration editor`,
        spellcheck: "false",
      }),
    ];

    if (placeholder) nextExtensions.push(editorPlaceholder(placeholder));

    if (readOnly) {
      nextExtensions.push(EditorState.readOnly.of(true));
    } else {
      nextExtensions.push(
        EditorView.updateListener.of((update: ViewUpdate) => {
          if (update.docChanged) {
            onChangeRef.current?.(update.state.doc.toString());
          }
        }),
      );
    }

    return nextExtensions;
  }, [language, placeholder, readOnly]);

  useEffect(() => {
    if (!containerRef.current) return;

    const state = EditorState.create({ doc: value, extensions });
    const view = new EditorView({ state, parent: containerRef.current });
    viewRef.current = view;

    const themeObserver = new MutationObserver(() => {
      view.dispatch({ effects: themeCompartment.reconfigure(getThemeExtensions()) });
    });
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

    return () => {
      themeObserver.disconnect();
      view.destroy();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [extensions]);

  // Sync external value changes
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const currentValue = view.state.doc.toString();
    if (currentValue !== value) {
      view.dispatch({
        changes: { from: 0, to: currentValue.length, insert: value },
      });
    }
  }, [value]);

  return (
    <div
      className="code-editor-wrapper flex flex-col overflow-hidden rounded-md border border-border bg-[var(--bg-input)] shadow-[var(--shadow-xs)] focus-within:border-[var(--border-strong)] focus-within:ring-2 focus-within:ring-primary/10"
      style={{
        minHeight,
        maxHeight,
      }}
    >
      <div className="flex h-8 shrink-0 items-center justify-between border-b border-border bg-[var(--bg-elevated)]/65 px-3">
        <span className="text-[10px] font-semibold uppercase text-muted-foreground">{language}</span>
        <span className="size-1.5 rounded-full bg-[var(--success)] opacity-75" aria-hidden="true" />
      </div>
      <div ref={containerRef} className="min-h-0 flex-1" />
    </div>
  );
}

export default memo(CodeEditorComponent);
