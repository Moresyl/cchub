# CCHub 项目约定

## 文件大小硬性约束

**任何源代码文件（.ts / .tsx / .rs）行数不得超过 900 行。**

- 这条规则没有例外。新增功能、修改现有逻辑时，如果会让文件超过 900 行，必须立即拆分。
- 拆分原则：
  - **TypeScript / TSX**：按 UI 组件、handlers、helpers、hooks 等关注点拆出独立文件，放到同名子目录（如 `src/pages/Foo/`、`src/components/Foo/`）下。
  - **Rust**：按域 / 数据流拆成 sub-module，主模块通过 `mod xxx; pub use xxx::*;` 聚合。
- 拆分后必须确保 `npx tsc --noEmit`、`cargo build`、相关测试全部通过，再提交。

## 项目运行

```bash
pnpm tauri dev      # 开发：前端 + Tauri 后端一起跑
pnpm dev            # 仅前端
pnpm tauri build    # 生产打包（.exe / .msi）
pnpm lint           # ESLint（0 警告）
pnpm test           # Vitest 单元测试
```

包管理器锁定 `pnpm`（10.32.1），不要用 npm / yarn。

## 技术栈

- Tauri v2 + React 19 + TypeScript + Vite 6 + Rust
- shadcn/ui 是默认 UI 组件库（Button / Input / Select / Dialog / Card 等必须用 shadcn/ui，不用原生 HTML 元素 + 手写 CSS 类）

## 提交规范

- 作者必须是 `Moresl`，禁止 `Co-Authored-By` / `Codex` / `anthropic` 等 AI 贡献者标记
- 中文 commit message，格式 `类型: 简短描述`（如 `重构:`、`修复:`、`性能:`）
- 不要在 commit message 里堆细节，简洁说清"做了什么"即可

## 版本号同步位置

升版本时这三处必须一起改：
- `package.json` → `"version"`
- `src-tauri/tauri.conf.json` → `"version"`
- `src-tauri/Cargo.toml` → `version`

或直接跑 `pnpm sync:version`。

## 版本发布说明

- 每次推送版本 Tag 并发布 GitHub Release 时，必须为该版本填写完整、可长期追溯的发布说明，禁止只写版本号或留空。
- 发布说明至少包含“新增/更新”和“问题修复”两部分；没有对应内容时明确写“无”，不能省略。
- 发布前根据该版本的实际提交更新 `.github/workflows/release.yml` 中的 `releaseBody`，不得沿用上一版本的说明。
- 发布完成后必须打开对应 Tag 的 GitHub Release 页面，核对版本号、说明内容和各平台安装包均正确后，才算发布完成。
