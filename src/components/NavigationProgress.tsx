import { useEffect, useRef, useState } from "react";
import { useLocation } from "react-router-dom";
import { useIsFetching } from "@tanstack/react-query";

/**
 * 顶部路由切换进度条：
 * - 路径变化 → 立即显示
 * - react-query isFetching 归零 → 延迟 150ms 后隐藏
 * - 最短展示 300ms，避免快速切换时闪烁
 */
export default function NavigationProgress() {
  const location = useLocation();
  const isFetching = useIsFetching();
  const [visible, setVisible] = useState(false);
  const shownAtRef = useRef(0);
  const hideTimerRef = useRef<number | null>(null);

  // 路径变化 → 立即显示
  useEffect(() => {
    if (hideTimerRef.current !== null) {
      window.clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
    shownAtRef.current = performance.now();
    setVisible(true);
  }, [location.pathname]);

  // isFetching 归零 → 考虑隐藏
  useEffect(() => {
    if (!visible) return;
    if (isFetching > 0) return;

    const elapsed = performance.now() - shownAtRef.current;
    const MIN_DISPLAY = 300;
    const SETTLE_DELAY = 150;
    const delay = Math.max(SETTLE_DELAY, MIN_DISPLAY - elapsed);

    if (hideTimerRef.current !== null) {
      window.clearTimeout(hideTimerRef.current);
    }
    hideTimerRef.current = window.setTimeout(() => {
      setVisible(false);
      hideTimerRef.current = null;
    }, delay);

    return () => {
      if (hideTimerRef.current !== null) {
        window.clearTimeout(hideTimerRef.current);
        hideTimerRef.current = null;
      }
    };
  }, [isFetching, visible]);

  return <div className={`nav-progress${visible ? " nav-progress-active" : ""}`} aria-hidden="true" />;
}
