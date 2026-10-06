import { Trash2, X } from "lucide-react";
import HighlightedText from "../../components/HighlightedText";
import { Button } from "../../components/ui/button";
import { formatTokenCount, type SessionSummary } from "./helpers";

interface DetailsHeaderProps {
  session: SessionSummary;
  query: string;
  locale: string;
  deleting: boolean;
  onDelete: () => void;
  onClose: () => void;
}

export default function DetailsHeader({ session, query, locale, deleting, onDelete, onClose }: DetailsHeaderProps) {
  const text = (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const deleteLabel = text("删除会话", "Delete session", "会話を削除");
  const closeLabel = text("关闭详情", "Close details", "詳細を閉じる");
  const stats = [
    session.tokens_used != null ? `${formatTokenCount(session.tokens_used)} tokens` : null,
    session.input_tokens != null ? `${text("输入", "Input", "入力")} ${formatTokenCount(session.input_tokens)}` : null,
    session.output_tokens != null
      ? `${text("输出", "Output", "出力")} ${formatTokenCount(session.output_tokens)}`
      : null,
  ].filter(Boolean);
  return (
    <div className="mb-3 space-y-2">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 flex-wrap items-center gap-1.5">
          <span className="badge badge-accent text-[11px]">{session.tool_name}</span>
          <span className="badge badge-muted text-[11px]">{session.source_kind}</span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={!session.can_delete || deleting}
            aria-label={deleteLabel}
            title={deleteLabel}
            onClick={onDelete}
            className="text-[var(--danger)]"
          >
            <Trash2 size={14} />
          </Button>
          <Button variant="ghost" size="icon-sm" aria-label={closeLabel} title={closeLabel} onClick={onClose}>
            <X size={14} />
          </Button>
        </div>
      </div>
      <h3 className="line-clamp-2 break-words text-base font-semibold leading-snug" title={session.title}>
        <HighlightedText text={session.title} query={query} />
      </h3>
      {stats.length > 0 && <p className="text-[11px] leading-relaxed text-muted-foreground">{stats.join(" · ")}</p>}
      {session.created_at && <p className="break-words text-[11px] text-muted-foreground">{session.created_at}</p>}
    </div>
  );
}
