interface HighlightedTextProps {
  text: string;
  query: string;
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export default function HighlightedText({ text, query }: HighlightedTextProps) {
  const trimmed = query.trim();
  if (!trimmed) {
    return <>{text}</>;
  }

  const matcher = new RegExp(`(${escapeRegExp(trimmed)})`, "ig");
  const parts = text.split(matcher);
  const normalized = trimmed.toLowerCase();

  return (
    <>
      {parts.map((part, index) => (
        part.toLowerCase() === normalized
          ? (
            <mark
              key={`${part}-${index}`}
              style={{
                background: "color-mix(in srgb, var(--warning) 24%, transparent)",
                color: "inherit",
                padding: 0,
                borderRadius: 2,
              }}
            >
              {part}
            </mark>
          )
          : <span key={`${part}-${index}`}>{part}</span>
      ))}
    </>
  );
}
