import type { Locale } from "../types/preferences";

// Only inspect complete lines outside fenced code. A code example can contain
// a language marker or heading without turning into release-note metadata.
function metadataLines(body: string) {
  let fence: { character: string; length: number } | null = null;
  let offset = 0;
  const lines: { value: string; start: number; end: number }[] = [];
  for (const line of body.split(/(?<=\n)/)) {
    const value = line.trim();
    const match = /^(`{3,}|~{3,})(.*)$/.exec(value);
    if (match) {
      if (!fence) fence = { character: match[1][0], length: match[1].length };
      else if (match[1][0] === fence.character && match[1].length >= fence.length && !match[2].trim()) fence = null;
    } else if (!fence) lines.push({ value, start: offset, end: offset + line.length });
    offset += line.length;
  }
  return lines;
}

export function localizedReleaseNotes(body: string | null | undefined, locale: Locale): string {
  const source = body?.trim() ?? "";
  if (!source) return "";
  const lines = metadataLines(source);
  const markers = lines.flatMap((line) => {
    const match = /^<!--\s*lang:(zh|en|ja)\s*-->$/.exec(line.value);
    return match ? [{ ...line, locale: match[1] as Locale }] : [];
  });
  if (markers.length) {
    const sections = new Map<Locale, string>();
    const initial = source.slice(0, markers[0].start).trim();
    if (initial) sections.set("en", initial);
    markers.forEach((marker, index) => {
      const content = source.slice(marker.end, markers[index + 1]?.start ?? source.length).trim();
      if (content) sections.set(marker.locale, [sections.get(marker.locale), content].filter(Boolean).join("\n\n"));
    });
    return sections.get(locale) || sections.get("en") || sections.get("zh") || sections.get("ja") || "";
  }

  // Historical public releases put Chinese first, an English summary second
  // and the shared installation table last. Keep that table in either view.
  const english = lines.find((line) => /^##\s+English Summary\s*$/i.test(line.value));
  if (!english) return source;
  const download = lines.find(
    (line) => line.start > english.start && /^##\s+下载\s*\/\s*Download\s*$/.test(line.value),
  );
  const chinese = source.slice(0, english.start).trim();
  const translated = source.slice(english.end, download?.start ?? source.length).trim();
  const selected = locale === "zh" ? chinese || translated : translated || chinese;
  const shared = download ? source.slice(download.start).trim() : "";
  return [selected, shared].filter(Boolean).join("\n\n");
}
