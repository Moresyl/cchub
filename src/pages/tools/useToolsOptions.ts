import { useMemo } from "react";
import { PERM_LEVELS, type HudDisplayBooleanKey, type HudGitStatusKey, type Hello2ccSelectKey } from "./helpers";
import { type Hello2ccSelectOption } from "../../components/Hello2ccSelectField";

type UiText = (zh: string, en: string, ja?: string) => string;

/**
 * 把 Tools 页面 200+ 行的下拉/复选/单选项 useMemo 集中到一个 hook，
 * Tools.tsx 仅需 useToolsOptions(uiText, tab) 即可拿到所有选项数据。
 */
export function useToolsOptions(uiText: UiText, tab: "claude" | "codex") {
  const unavailableLabel = useMemo(() => uiText("未安装", "N/A", "未インストール"), [uiText]);
  const permLevelLabels = useMemo(
    () => PERM_LEVELS.map((level) => uiText(level.label_zh, level.label_en, level.label_ja)),
    [uiText],
  );
  const autoUpdateOptions = useMemo(
    () => [
      { value: "latest", label: uiText("最新", "Latest", "最新") },
      { value: "stable", label: uiText("稳定", "Stable", "安定版") },
      { value: "disabled", label: uiText("关闭", "Off", "オフ") },
    ],
    [uiText],
  );
  const claudeModelOptions = useMemo(
    () => [
      { value: "opus", label: "Opus" },
      { value: "sonnet", label: "Sonnet" },
      { value: "haiku", label: "Haiku" },
    ],
    [],
  );
  const hudLayoutOptions = useMemo(
    () => [
      { value: "expanded", label: uiText("多行展开", "Expanded", "展開表示"), style: { fontSize: 11 } },
      { value: "compact", label: uiText("单行紧凑", "Compact", "コンパクト"), style: { fontSize: 11 } },
    ],
    [uiText],
  );
  const hudPathLevelOptions = useMemo(
    () => [1, 2, 3].map((value) => ({ value, label: String(value), style: { fontSize: 11, minWidth: 24 } })),
    [],
  );
  const hudContextValueOptions = useMemo(
    () => [
      { value: "percent", label: "45%", style: { fontSize: 11 } },
      { value: "tokens", label: "45k/200k", style: { fontSize: 11 } },
      { value: "remaining", label: uiText("剩余", "Remain", "残り"), style: { fontSize: 11 } },
      { value: "both", label: uiText("全部", "Both", "両方"), style: { fontSize: 11 } },
    ],
    [uiText],
  );
  const codexApprovalOptions = useMemo(
    () => [
      { value: "suggest", label: uiText("建议", "Suggest", "提案") },
      { value: "auto-edit", label: uiText("自动编辑", "Auto Edit", "自動編集") },
      { value: "full-auto", label: uiText("全自动", "Full Auto", "フルオート") },
    ],
    [uiText],
  );
  const codexReasoningOptions = useMemo(
    () => [
      { value: "low", label: uiText("低", "Low", "低") },
      { value: "medium", label: uiText("中", "Medium", "中") },
      { value: "high", label: uiText("高", "High", "高") },
      { value: "xhigh", label: uiText("极高", "XHigh", "最高") },
    ],
    [uiText],
  );
  const permLevelOptions = useMemo(
    () => PERM_LEVELS.map((level, index) => ({ value: index, label: permLevelLabels[index], color: level.color })),
    [permLevelLabels],
  );
  const hudGitStatusOptions = useMemo(
    () => [
      { key: "enabled" as HudGitStatusKey, label: uiText("显示分支", "Branch", "ブランチ表示"), defaultValue: true },
      {
        key: "showDirty" as HudGitStatusKey,
        label: uiText("未提交标记", "Dirty Mark", "変更あり表示"),
        defaultValue: true,
      },
      {
        key: "showAheadBehind" as HudGitStatusKey,
        label: uiText("领先/落后", "Ahead/Behind", "先行/遅延"),
        defaultValue: false,
      },
      {
        key: "showFileStats" as HudGitStatusKey,
        label: uiText("文件统计", "File Stats", "ファイル統計"),
        defaultValue: false,
      },
    ],
    [uiText],
  );
  const hudDisplayOptions = useMemo(
    () => [
      { key: "showModel" as HudDisplayBooleanKey, label: uiText("模型名", "Model", "モデル名"), defaultValue: true },
      {
        key: "showProject" as HudDisplayBooleanKey,
        label: uiText("项目路径", "Project Path", "プロジェクトパス"),
        defaultValue: true,
      },
      {
        key: "showContextBar" as HudDisplayBooleanKey,
        label: uiText("上下文进度条", "Context Bar", "コンテキストバー"),
        defaultValue: true,
      },
      {
        key: "showConfigCounts" as HudDisplayBooleanKey,
        label: uiText("配置计数", "Config Counts", "設定数"),
        defaultValue: false,
      },
      {
        key: "showDuration" as HudDisplayBooleanKey,
        label: uiText("会话时长", "Duration", "継続時間"),
        defaultValue: false,
      },
      {
        key: "showSpeed" as HudDisplayBooleanKey,
        label: uiText("输出速度", "Output Speed", "出力速度"),
        defaultValue: false,
      },
      { key: "showUsage" as HudDisplayBooleanKey, label: uiText("用量限制", "Usage", "使用量"), defaultValue: true },
      {
        key: "usageBarEnabled" as HudDisplayBooleanKey,
        label: uiText("用量进度条", "Usage Bar", "使用量バー"),
        defaultValue: true,
      },
      {
        key: "showTokenBreakdown" as HudDisplayBooleanKey,
        label: uiText("Token 明细", "Token Detail", "トークン詳細"),
        defaultValue: true,
      },
      {
        key: "showTools" as HudDisplayBooleanKey,
        label: uiText("工具活动", "Tools", "ツール活動"),
        defaultValue: false,
      },
      {
        key: "showAgents" as HudDisplayBooleanKey,
        label: uiText("Agent 活动", "Agents", "Agent 活動"),
        defaultValue: false,
      },
      {
        key: "showTodos" as HudDisplayBooleanKey,
        label: uiText("Todo 进度", "Todos", "Todo 進捗"),
        defaultValue: false,
      },
      {
        key: "showSessionName" as HudDisplayBooleanKey,
        label: uiText("会话名称", "Session Name", "セッション名"),
        defaultValue: false,
      },
      {
        key: "showClaudeCodeVersion" as HudDisplayBooleanKey,
        label: uiText("CC 版本号", "CC Version", "CC バージョン"),
        defaultValue: false,
      },
      {
        key: "showMemoryUsage" as HudDisplayBooleanKey,
        label: uiText("内存占用", "Memory Usage", "メモリ使用量"),
        defaultValue: false,
      },
    ],
    [uiText],
  );
  const hello2ccRoutingOptions = useMemo<Hello2ccSelectOption[]>(
    () => [
      { value: "native-inject", label: "native-inject" },
      { value: "prompt-only", label: "prompt-only" },
    ],
    [],
  );
  const hello2ccCompatibilityOptions = useMemo<Hello2ccSelectOption[]>(
    () => [
      { value: "full", label: "full" },
      { value: "sanitize-only", label: "sanitize-only" },
    ],
    [],
  );
  const hello2ccModelOptions = useMemo<Hello2ccSelectOption[]>(
    () => [
      { value: "", label: uiText("留空", "Blank", "空欄") },
      { value: "inherit", label: "inherit" },
      { value: "opus", label: "opus" },
      { value: "sonnet", label: "sonnet" },
      { value: "haiku", label: "haiku" },
    ],
    [uiText],
  );
  const hello2ccModelFields = useMemo(
    () => [
      {
        fieldKey: "default_agent_model" as Hello2ccSelectKey,
        label: uiText("默认 Agent 槽位", "Default Agent Slot", "既定 Agent スロット"),
        description: uiText("统一默认值", "Global default", "全体デフォルト"),
      },
      {
        fieldKey: "primary_model" as Hello2ccSelectKey,
        label: uiText("Primary Model", "Primary Model", "Primary Model"),
        description: uiText("高能力 Agent", "High-capability agents", "高能力 Agent"),
      },
      {
        fieldKey: "subagent_model" as Hello2ccSelectKey,
        label: uiText("Subagent Model", "Subagent Model", "Subagent Model"),
        description: uiText("未指定模型的 Agent", "Agents without explicit model", "未指定モデルの Agent"),
      },
      {
        fieldKey: "guide_model" as Hello2ccSelectKey,
        label: uiText("Guide Model", "Guide Model", "Guide Model"),
        description: "Claude Code Guide",
      },
      {
        fieldKey: "explore_model" as Hello2ccSelectKey,
        label: uiText("Explore Model", "Explore Model", "Explore Model"),
        description: "Explore",
      },
      {
        fieldKey: "plan_model" as Hello2ccSelectKey,
        label: uiText("Plan Model", "Plan Model", "Plan Model"),
        description: "Plan",
      },
      {
        fieldKey: "general_model" as Hello2ccSelectKey,
        label: uiText("General Model", "General Model", "General Model"),
        description: "General-Purpose",
      },
      {
        fieldKey: "team_model" as Hello2ccSelectKey,
        label: uiText("Team Model", "Team Model", "Team Model"),
        description: uiText("团队 teammate", "Team teammates", "チーム teammate"),
      },
    ],
    [uiText],
  );
  const noVisibleTabsTitle = useMemo(
    () => uiText("当前已隐藏所有工具页签", "All tool tabs are currently hidden", "すべてのツールタブは現在非表示です"),
    [uiText],
  );
  const noVisibleTabsDescription = useMemo(
    () =>
      uiText(
        "可在设置页的 App 可见性中重新开启",
        "Re-enable them from Settings > App Visibility",
        "Settings > App Visibility から再表示できます",
      ),
    [uiText],
  );
  const notInstalledTitle = useMemo(
    () =>
      uiText(
        `${tab === "claude" ? "Claude Code" : "Codex CLI"} 未安装`,
        `${tab === "claude" ? "Claude Code" : "Codex CLI"} not installed`,
        `${tab === "claude" ? "Claude Code" : "Codex CLI"} は未インストールです`,
      ),
    [tab, uiText],
  );
  const notInstalledDescription = useMemo(
    () =>
      uiText(
        "安装后即可在此管理工具设置",
        "Install it to manage settings here",
        "インストール後にここで设置を管理できます",
      ),
    [uiText],
  );

  return {
    unavailableLabel,
    permLevelLabels,
    autoUpdateOptions,
    claudeModelOptions,
    hudLayoutOptions,
    hudPathLevelOptions,
    hudContextValueOptions,
    codexApprovalOptions,
    codexReasoningOptions,
    permLevelOptions,
    hudGitStatusOptions,
    hudDisplayOptions,
    hello2ccRoutingOptions,
    hello2ccCompatibilityOptions,
    hello2ccModelOptions,
    hello2ccModelFields,
    noVisibleTabsTitle,
    noVisibleTabsDescription,
    notInstalledTitle,
    notInstalledDescription,
  };
}

import { type Hello2ccConfigField } from "../../components/Hello2ccConfigSection";
import type { Hello2ccConfig } from "./helpers";

interface BuildHello2ccFieldsArgs {
  uiText: (zh: string, en: string, ja?: string) => string;
  hello2ccDraft: Hello2ccConfig;
  hello2ccRoutingOptions: Hello2ccSelectOption[];
  hello2ccCompatibilityOptions: Hello2ccSelectOption[];
  hello2ccModelFields: Array<{ fieldKey: Hello2ccSelectKey; label: string; description: string }>;
  hello2ccModelOptions: Hello2ccSelectOption[];
}

export function buildHello2ccSelectFields(args: BuildHello2ccFieldsArgs): Hello2ccConfigField[] {
  const {
    uiText,
    hello2ccDraft,
    hello2ccRoutingOptions,
    hello2ccCompatibilityOptions,
    hello2ccModelFields,
    hello2ccModelOptions,
  } = args;
  return [
    {
      fieldKey: "routing_policy",
      label: uiText("路由策略", "Routing Policy", "ルーティングポリシー"),
      description: uiText(
        "决定是否在原生 Agent 调用前注入模型槽位",
        "Choose whether native agent calls receive silent model injection",
        "ネイティブ Agent 呼び出し前にモデル注入するかを選びます",
      ),
      value: hello2ccDraft.routing_policy,
      options: hello2ccRoutingOptions,
    },
    {
      fieldKey: "compatibility_mode",
      label: uiText("兼容模式", "Compatibility Mode", "互換モード"),
      description: uiText(
        "与其他插件共存时可切到仅净化模式",
        "Use sanitize-only when coexisting with other orchestration plugins",
        "他プラグインと共存する場合は sanitize-only を使います",
      ),
      value: hello2ccDraft.compatibility_mode,
      options: hello2ccCompatibilityOptions,
    },
    ...hello2ccModelFields.map((field) => ({
      fieldKey: field.fieldKey,
      label: field.label,
      description: field.description,
      value: hello2ccDraft[field.fieldKey],
      options: hello2ccModelOptions,
    })),
  ];
}
