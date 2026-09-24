import HighlightedText from "../../components/HighlightedText";
import { entryBadgeColor, type SessionEntry } from "./helpers";

interface SessionEntriesProps {
  entries: SessionEntry[];
  query: string;
  emptyLabel: string;
}

export default function SessionEntries({ entries, query, emptyLabel }: SessionEntriesProps) {
  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      {entries.length === 0 ? (
        <div className="flex h-full items-center justify-center text-[13px] text-muted-foreground">{emptyLabel}</div>
      ) : (
        <div className="flex flex-col gap-2.5">
          {entries.map((entry) => (
            <section
              key={entry.id}
              id={`session-entry-${entry.id}`}
              className="rounded-md border border-border bg-card p-3"
            >
              <div className="mb-2 flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-1.5">
                  <span className={`badge ${entryBadgeColor(entry.kind)} text-[10px]`}>{entry.kind}</span>
                  <span className="min-w-0 truncate text-[12px] font-semibold" title={entry.title}>
                    <HighlightedText text={entry.title} query={query} />
                  </span>
                </div>
                {entry.timestamp && (
                  <span className="shrink-0 text-[10px] text-muted-foreground">{entry.timestamp}</span>
                )}
              </div>
              <pre className="m-0 max-h-80 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-normal text-muted-foreground">
                <HighlightedText text={entry.content} query={query} />
              </pre>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
