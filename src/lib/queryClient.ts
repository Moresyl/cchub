import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // 30s 内不重新发起请求；切回页面时仍可立刻拿到上次数据。
      staleTime: 30_000,
      // gcTime 控制无订阅查询的内存驻留时间。Tauri 桌面端不会像 web 那样
      // 长时间挂起，5 分钟足以覆盖用户来回切页面，同时避免长时间持有大对象
      // （例如 240 条 sessions、240 条 proxy_request_logs）。
      gcTime: 5 * 60_000,
      refetchOnWindowFocus: false,
      refetchOnReconnect: false,
      // mount 时如果有缓存就不重发，配合 prefetch / staleTime 让切页瞬开
      refetchOnMount: false,
      retry: 1,
      // 后端返回的数组里大部分对象是不变的，开启 structural sharing 让
      // useQuery 的 data 引用尽可能稳定，减少消费组件的级联重渲染。
      structuralSharing: true,
      // 默认网络模式 online —— Tauri 内嵌的 WebView 没有真正的网络层概念，
      // online 模式仍能正常调用 invoke()，无需切换。
    },
    mutations: {
      retry: 0,
      // mutation 一般在用户点击后即返回结果，没必要保留中间态太久
      gcTime: 60_000,
    },
  },
});
