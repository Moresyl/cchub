# 版本记录

## 1.7.2

### 新增 / 更新

- 托管账户配额显示上次成功读取时间和有效的重置时间，支持失败后单独重试。
- 统一界面中等与加粗字重为 500 / 600，大号控件高度统一为 36px。

### 变更与安全

- 配额刷新失败时保留同一登录账户的上次结果并明确标记；切换账户或重新登录后清除旧结果，避免混用。
- 代理规范化超过 64 个字符的工具调用标识，并保持调用与结果关联；不重写其他历史内容。
- 本次不新增第三方配置预设，不更改默认深色主题设置。

### 问题修复与打磨

- 修复下拉框长名称撑宽窄窗口的问题，保持选项换行、键盘选择和关闭后的焦点恢复。
- 统一复选框尺寸、圆角、选中、混合和禁用状态，修复全局焦点样式覆盖组件焦点提示的问题。
- 修复 Prompt 名称输入框的标签关联，辅助说明独立作为无障碍描述。
- 补充控件、配额刷新、账户隔离和代理调用配对的回归测试。

### 安装

- Windows 提供 NSIS 和 MSI；macOS 提供 Apple Silicon 和 Intel；Linux 提供 deb、rpm 和 AppImage。
- 可从应用内检查更新，或从对应版本的 Release 页面下载安装包。

### English summary

- Refines checkbox states, typography and narrow-window select layout.
- Retains clearly marked last successful account quota results on refresh failure, with retry and account isolation.
- Repairs oversized tool-call identifiers while preserving call/result pairing and unrelated history bytes.
- Corrects the Prompt name field's accessible label and expands regression coverage.

## 1.7.1

### 新增 / 更新

- 首次启动默认使用深色主题，保留用户明确选择的浅色或跟随系统设置。

### 问题修复

- 统一启动页与主界面的偏好读取，校验恢复的主题和语言值。
- 补充默认主题、旧设置兼容、异常值及跟随系统的回归测试。
