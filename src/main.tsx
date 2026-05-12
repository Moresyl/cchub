import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { initTheme } from "./lib/theme";
import "./styles/globals.css";

initTheme();

// 禁用浏览器缩放（Ctrl+滚轮 / Ctrl+加减 / Ctrl+0）。
// wheel 事件在滚动期间极高频触发，先做一次廉价的修饰键判断再考虑其余分支。
document.addEventListener(
  "keydown",
  (e) => {
    if (!(e.ctrlKey || e.metaKey)) return;
    const k = e.key;
    if (k === "+" || k === "-" || k === "=" || k === "0") {
      e.preventDefault();
    }
  },
  { passive: false },
);
document.addEventListener(
  "wheel",
  (e) => {
    // 99% 的滚轮事件没有修饰键，先 fast-fail。
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
  },
  { passive: false },
);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

// React 首次提交后淡出并移除启动动画
function hideSplash() {
  const splash = document.getElementById("cchub-splash");
  if (!splash) return;
  splash.classList.add("splash-hidden");
  setTimeout(() => splash.remove(), 300);
}
requestAnimationFrame(() => {
  requestAnimationFrame(hideSplash);
});
