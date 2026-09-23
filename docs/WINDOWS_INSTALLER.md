# Windows 安装器视觉规范

CCHub 的 NSIS 与 MSI 安装入口共用应用自身的图标、深色侧栏、浅色内容区和青色状态强调色。NSIS 卸载器使用独立头图与应用图标，并在删除本地数据前提供明确的复选项文案。

## 覆盖范围

| 安装器 | 视觉资产 | 交互与文案 |
| ------ | -------- | ---------- |
| NSIS | 164×314 侧栏、150×57 安装头图、150×57 卸载头图、安装/卸载图标 | 根据系统语言显示简体中文或英文；默认按当前用户安装 |
| MSI | 493×58 横幅、493×312 欢迎/完成页图 | 使用固定 Upgrade Code，保证后续版本可正确升级或删除 |

所有位图均为 Windows 安装器要求的 24 位 RGB BMP。源图来自 `src-tauri/icons/icon.png`，没有运行时网络依赖。

## 重新生成

在 Windows PowerShell 中运行：

```powershell
pnpm generate:installer-assets
```

生成脚本为 `scripts/generate-installer-assets.ps1`，产物位于 `src-tauri/installer/`。提交前运行：

```powershell
pnpm check:installer
pnpm tauri build --bundles nsis msi
```

`check:installer` 会检查 Tauri CLI 版本、NSIS/MSI 配置、双语文件、位图尺寸和 24 位像素格式。普通前端构建也会在 `prebuild` 阶段执行同一校验。

## 数据处理

- 覆盖安装和自动更新不会删除配置数据库或本地设置。
- NSIS 卸载页默认保留本地数据；只有用户主动勾选“同时删除 CCHub 的本地数据与设置”时才删除。
- MSI 的 Upgrade Code 固定为 `0f196973-5299-5133-b433-da6f8f0cbd91`，不得在后续版本中随意更改。
