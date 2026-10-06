# 版本记录

## 未发布

### 新增 / 更新

- 普通响应和失败流重试的计费写入改为独立任务，数据库操作在阻塞工作线程执行，响应和重试仍等待记账结果。
- 在请求前预留记账容量；客户端取消等待时，已经提交的写入任务继续执行并持有容量，限制待写任务积累。

### 问题修复

- 失败流的父记录写入失败时停止请求下一家供应商，避免把本次用量追加到旧的请求记录。
- 记账时间在提交任务时捕获，避免数据库排队跨日改变记录日期；待写记录不携带请求认证头和请求正文覆盖内容。

## 1.7.9

### 新增 / 更新

- 价格设置冲突增加应用内核对窗口，对照最初载入、已保存设置、我的草稿及核对结果。
- 不同模型的修改自动合并，同一模型的不同修改提供保留草稿或已保存设置的选择。
- 模型更改支持搜索、冲突筛选和完整分页，更新中英文说明与实际组件演示截图。

### 变更与安全

- 核对读取最新设置，应用结果后仍需保存；保存再次核对版本，期间再次变化时停止覆盖并保留草稿。
- 读取失败保留草稿并提供安全重试，取消、重试及离开页面后忽略旧读取结果。
- 合并超出选项上限时阻止应用，不静默截断选择。

### 问题修复与打磨

- 设置冲突可以核对合并，不再只能放弃草稿后重载。
- 核对窗口采用统一卡片、输入框、选择框与弹窗，窄窗口使用双列对照和完整高度布局。
- 应用或取消后恢复编辑焦点；冲突筛选保留已解决条目，方便再次调整并避免操作控件消失。
- 补充三方合并、读取所有权、冲突后保存、长标识分页与键盘选择回归测试。

### 安装

- Windows 提供 NSIS 和 MSI；macOS 提供 Apple Silicon 和 Intel；Linux 提供 deb、rpm 和 AppImage。
- 可从应用内检查更新，或从对应版本的 Release 页面下载安装包。

### English summary

- Adds pricing preference review with initial, saved, draft and resulting values; independent model changes merge automatically and conflicts require a choice.
- Supports search, conflict filtering and complete pagination, with shared controls and a compact comparison layout.
- Rechecks settings when saving, retains drafts after failures or repeated conflicts, rejects stale reads and oversized merges, and updates documentation, the screenshot and regression tests.

## 1.7.8

### 新增 / 更新

- 模型价格设置使用统一草稿、保存与保存后同步入口，刷新状态和切换页面保留未保存选项。
- 模型目录增加完整分页、独立字段标签、价格单位说明与长名称换行，显示输入、输出及可用的缓存价格。
- 更新中英文价格设置说明与实际组件演示截图。

### 问题修复

- 常用模型标记由后端提供，修复界面勾选范围与实际同步范围不一致的问题。
- 保存核对读取时的选项并保留较新的同步状态；同步请求完成后再次核对选项，防止覆盖期间的更改。
- 同步统一选择重复价格标识的条目，导入数量按实际唯一模型计算，不再在文件与数据库中采用不同价格。
- 读取或保存失败保留草稿，提供安全的重试提示；确认放弃草稿后仅在重新读取成功时替换。
- 修复关闭确认弹窗后的焦点丢失，补充价格选择、保存顺序、异步结果和设置冲突回归测试。

### 变更与安全

- 读取价格设置不再触发价格文件同步；设置读取失败时阻止保存默认值，错误提示不回显内部详情。
- 保存和同步均核对已加载的设置，发生冲突时停止覆盖并保留草稿。

### 安装

- Windows 提供 NSIS 和 MSI；macOS 提供 Apple Silicon 和 Intel；Linux 提供 deb、rpm 和 AppImage。
- 可从应用内检查更新，或从对应版本的 Release 页面下载安装包。

### English summary

- Preserves pricing drafts across refreshes and page changes, with explicit save and save-and-sync actions.
- Adds complete catalog pagination, clear field labels, price units and wrapped model names.
- Uses backend common-model markers, rejects changed settings before writes, preserves newer sync status and consistently deduplicates imported prices.
- Keeps drafts after failures, restores confirmation-dialog focus, and updates bilingual documentation, the screenshot and regression tests.

## 1.7.7

### 新增 / 更新

- 用量供应商与模型排名增加分页；窄区域用卡片完整展示指标，宽区域保留表格。
- 每日趋势限制滚动高度，长名称换行，摘要数字统一半粗字重。
- 更新中英文用量分析说明及实际页面组件演示截图。

### 问题修复

- 修复快速切换筛选时旧响应覆盖当前结果，以及加载时筛选栏消失的问题。
- 合并连续用量事件，避免同一筛选重复并发刷新；离开页面后清理延迟刷新和事件监听。
- 刷新失败保留同一筛选下的上次成功数据，提供安全的恢复提示；事件监听不可用时仍可手动刷新。
- 保留较宽范围的供应商和模型选项，避免选择一项后其他选项消失；切换应用或时间范围不会沿用旧范围选项。
- 无请求的日期不再显示非零趋势条，超过十二条的排名不再被静默隐藏。

### 安装

- Windows 提供 NSIS 和 MSI；macOS 提供 Apple Silicon 和 Intel；Linux 提供 deb、rpm 和 AppImage。
- 可从应用内检查更新，或从对应版本的 Release 页面下载安装包。

### English summary

- Adds ranking pagination and compact cards for narrow panels, with wrapped names and a bounded trend area.
- Rejects stale filter responses, keeps filters available while loading, and coalesces live refreshes.
- Preserves same-filter results after refresh failures, handles unavailable event listeners, and updates bilingual documentation and the screenshot.

## 1.7.6

### 新增 / 更新

- 指令、记忆与发布说明共用 Markdown 正文样式，统一标题、列表、引用、代码与表格层次。
- 只读任务列表使用统一勾选框，表格增加带标签的键盘滚动区域。
- 更新中英文指令预览说明与实际组件演示截图。

### 变更与安全

- Prompt 文件冲突、编码与权限错误使用本地化恢复提示，不回显原始内部错误。

### 问题修复与打磨

- 修复记忆正文缺少 Markdown 样式、标题与列表退化为普通文本的问题。
- 修复已有超长说明仍可点击保存却没有反馈的问题，明确标记错误并阻止按钮和快捷键提交。
- 修复说明输入的 UTF-16 长度上限提前截断补充 Unicode 字符的问题，按字符数与后端保持一致。
- 修复说明字段辅助提示被包含在无障碍名称中的问题，关联独立名称与说明；补充指令编辑器及只读预览标签。
- 补充 Markdown 语义、自定义渲染、安全链接、说明长度与文件冲突回归测试。

### 安装

- Windows 提供 NSIS 和 MSI；macOS 提供 Apple Silicon 和 Intel；Linux 提供 deb、rpm 和 AppImage。
- 可从应用内检查更新，或从对应版本的 Release 页面下载安装包。

### English summary

- Unifies Markdown typography across instructions, memory and release notes, with shared task checkboxes and keyboard-focusable tables.
- Validates description limits before saving, handles supplementary Unicode characters and improves accessible field labels.
- Localizes conflict and file-error recovery without exposing internal details, and updates bilingual documentation and screenshots.

## 1.7.5

### 新增 / 更新

- 配置文件页支持 OpenClaw 原生供应商、模型、别名及主模型与回退模型编辑；结构化字段和 JSON5 高亮编辑器共用文件草稿与保存入口。
- 记忆与日志使用独立搜索和全文预览窗口，支持错误重试与窄窗口布局。
- 更新中英文配置文件说明与实际组件演示截图。

### 变更与安全

- 原生文件解析与字段合并只更新当前草稿，通过统一保存入口写入；保存失败的提示不显示内部配置详情。
- 无效 JSON5、重复字段与非法数值会阻止保存，避免写入空的默认配置。

### 问题修复

- 修复快捷表单按配置片段重建整份原生文件而丢失无关配置的问题；字段编辑保留 JSON5 注释、原有格式、凭据引用及扩展字段。
- 修复记忆搜索和记录选择重读配置、覆盖未保存修改的问题，忽略过期搜索和读取结果。
- 文件保存核对加载时的原始内容；外部修改或删除时停止覆盖，保留草稿并提示重新加载核对。
- 修复模型数组内的字段编辑、删除模型后的数值路径及空白数值覆盖；无效容器和数值不会静默转换或覆盖原值。

### 安装

- Windows 提供 NSIS 和 MSI；macOS 提供 Apple Silicon 和 Intel；Linux 提供 deb、rpm 和 AppImage。
- 可从应用内检查更新，或从对应版本的 Release 页面下载安装包。

### English summary

- Adds shared native OpenClaw fields and a JSON5 editor with one draft and save action.
- Separates memory search and preview from configuration loading, preserving drafts and rejecting stale results.
- Retains comments, credential references and extension fields, and checks the original file contents before saving.
- Improves numeric-field validation, array edits, save-conflict recovery, bilingual documentation and screenshots.

## 1.7.4

### 新增 / 更新

- 重整 OpenClaw 配置工作区，统一输入、选择器、标签页和底部保存栏；支持模型别名、撤销修改和键盘标签导航。
- 更新中英文配置工作区说明与实际页面演示截图。

### 变更与安全

- 读取失败时阻止保存空默认值；保存期间锁定编辑和重复提交，错误提示不展示内部配置详情。

### 问题修复

- 区分配置读取失败、健康检查失败和未检测到配置文件，提供独立重试。
- 配置读取失败后禁止保存空默认值，保存失败和标签切换后保留草稿。
- 保留结构化环境设置的原始类型，以及工具、Agent 和模型配置中的扩展字段。
- 避免变量重名覆盖已有设置，防止保存中重复提交及修改；检查状态时配置暂时不可用也保留草稿。

### 安装

- Windows 提供 NSIS 和 MSI；macOS 提供 Apple Silicon 和 Intel；Linux 提供 deb、rpm 和 AppImage。
- 可从应用内检查更新，或从对应版本的 Release 页面下载安装包。

### English summary

- Refines the OpenClaw workspace with consistent controls, model aliases, undo and keyboard tab navigation.
- Preserves drafts across tab changes and failed saves, and separates configuration and health-check failures with retries.
- Preserves structured environment values and extension fields, validates duplicate keys and prevents duplicate saves.
- Updates bilingual documentation and the workspace screenshot.

## 1.7.3

### 新增 / 更新

- OpenCode 配置同时兼容新版 `providers` 与旧版 `provider`，支持原生设置、模型数组变体和对象形式的模型选择。
- 原生配置使用语法高亮编辑器，保留扩展字段；配置卡片显示选中模型、变体及对应连接地址。
- 更新中英文原生配置说明与配置编辑器演示截图。

### 变更与安全

- 保存前校验原生字段；共享组包含无效配置时停止入库，避免只保存部分成员。
- 连接检查、用量凭据和代理转发统一解析选中模型与变体的设置、请求头和请求参数。
- 代理接管清除模型与变体的地址和认证覆盖，避免绕过本地端点；最小流式检查保持测试模型、提示词及输出上限。

### 问题修复与打磨

- 修复原生配置被旧版表单重建后丢失模型变体与扩展字段的问题。
- 修复编辑原生共享配置时覆盖其他工具配置内容的问题，保存失败后继续保留草稿供重试。
- 修复原生供应商 ID 被裁剪后生成重复条目的问题，保留原 ID。
- 修复有效原生条目的优先级、无效条目回退及被遮蔽旧配置的写入提示。
- 为配置名称和工具选择关联无障碍标签，锁定已保存原生配置的工具类型。
- 补充文件往返、数据库保存、四种协议流式请求、代理接管及编辑器交互回归测试。

### 安装

- Windows 提供 NSIS 和 MSI；macOS 提供 Apple Silicon 和 Intel；Linux 提供 deb、rpm 和 AppImage。
- 可从应用内检查更新，或从对应版本的 Release 页面下载安装包。

### English summary

- Supports native OpenCode provider settings, model variants and object-based model selection alongside legacy profiles.
- Preserves native extensions and other shared tools' configuration contents during editing.
- Resolves selected-model connection overlays consistently and prevents native overrides from bypassing local proxy takeover.
- Corrects exact provider ID handling and accessible editor labels; updates documentation, screenshots and regression coverage.

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
