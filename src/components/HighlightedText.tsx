import { memo, useMemo } from "react";

interface HighlightedTextProps {
  text: string;
  query: string;
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// 同一 query 在大列表中会被 N 个 HighlightedText 同时引用（Sessions 240 行 × 4 字段
// = 960 次），用进程内的轻量 cache 让多个组件实例共享同一编译好的 RegExp，避免
// 每个实例各自跑一次 escapeRegExp + new RegExp。
const matcherCache = new Map<string, { matcher: RegExp; normalized: string }>();
const MATCHER_CACHE_LIMIT = 32;

function getMatcher(trimmedQuery: string) {
  let entry = matcherCache.get(trimmedQuery);
  if (!entry) {
    entry = {
      matcher: new RegExp(`(${escapeRegExp(trimmedQuery)})`, "ig"),
      normalized: trimmedQuery.toLowerCase(),
    };
    if (matcherCache.size >= MATCHER_CACHE_LIMIT) {
      // 简单 FIFO 淘汰，搜索输入一般不会涉及更多并发 query
      const firstKey = matcherCache.keys().next().value;
      if (firstKey !== undefined) matcherCache.delete(firstKey);
    }
    matcherCache.set(trimmedQuery, entry);
  }
  return entry;
}

const markStyle = {
  background: "color-mix(in srgb, var(--warning) 24%, transparent)",
  color: "inherit",
  padding: 0,
  borderRadius: 2,
} as const;

function HighlightedTextComponent({ text, query }: HighlightedTextProps) {
  // 没有查询时直接返回，是热路径中的快速通道
  const trimmed = query.trim();
  // 计算结果按 (text, trimmed) 缓存，相同 props 重渲染零开销
  return useMemo(() => {
    if (!trimmed) return <>{text}</>;

    const { matcher, normalized } = getMatcher(trimmed);
    // String.prototype.split 在 V8 中对带 capturing group 的 regex 已经较快，
    // 但全局 regex 需要 reset lastIndex 防止跨调用污染
    matcher.lastIndex = 0;
    const parts = text.split(matcher);

    return (
      <>
        {parts.map((part, index) =>
          part.toLowerCase() === normalized ? (
            <mark key={index} style={markStyle}>
              {part}
            </mark>
          ) : (
            <span key={index}>{part}</span>
          ),
        )}
      </>
    );
  }, [text, trimmed]);
}

export default memo(HighlightedTextComponent);
