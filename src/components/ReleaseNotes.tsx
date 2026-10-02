import { useMemo } from "react";
import { open } from "@tauri-apps/plugin-shell";
import type { Components } from "react-markdown";
import { usePreferences } from "../stores/preferences";
import MarkdownPreview from "./MarkdownPreview";
import { showToast } from "./Toast";

export default function ReleaseNotes({ content }: { content: string }) {
  const locale = usePreferences((state) => state.locale);
  const components = useMemo<Components>(
    () => ({
      a: ({ href, children }) => {
        if (!href || !/^https?:\/\//i.test(href)) return <span>{children}</span>;
        return (
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            onClick={(event) => {
              event.preventDefault();
              void open(href).catch(() =>
                showToast(
                  "error",
                  locale === "zh"
                    ? "无法打开链接，请稍后重试"
                    : locale === "ja"
                      ? "リンクを開けませんでした。もう一度お試しください"
                      : "Could not open the link. Please try again.",
                ),
              );
            }}
          >
            {children}
          </a>
        );
      },
      img: ({ alt }) => <span>{alt}</span>,
    }),
    [locale],
  );
  return (
    <MarkdownPreview
      content={content}
      components={components}
      loadingLabel={
        locale === "zh" ? "正在加载更新说明…" : locale === "ja" ? "更新内容を読み込み中…" : "Loading release notes…"
      }
    />
  );
}
