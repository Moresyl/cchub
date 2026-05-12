import { Suspense, lazy } from "react";

import Hello2ccConfigSection, { type Hello2ccConfigField } from "../../components/Hello2ccConfigSection";
import ToolsCheckboxRow from "../../components/ToolsCheckboxRow";
import ToolsCheckboxSection from "../../components/ToolsCheckboxSection";
import ToolsChoiceCard from "../../components/ToolsChoiceCard";
import ToolsChoiceRow from "../../components/ToolsChoiceRow";
import ToolsManagedSectionHeader from "../../components/ToolsManagedSectionHeader";
import ToolsPermissionCard from "../../components/ToolsPermissionCard";
import ToolsToggleCard from "../../components/ToolsToggleCard";

const ProxyAdvancedPanel = lazy(() => import("../ProxyAdvanced"));

type UiText = (zh: string, en: string, ja?: string) => string;

/* eslint-disable @typescript-eslint/no-explicit-any */
export interface ClaudeTabProps {
  uiText: UiText;
  perm: { color: string; label_zh: string; label_en: string; label_ja: string };
  permLevel: number;
  permDescription: string;
  permLevelOptions: any[];
  handleSelectPermLevel: (value: string | number) => void;
  handleChangePermLevelRange: any;
  handleCommitPermLevelPointerUp: any;
  handleCommitPermLevelKeyUp: any;
  handleCommitPermLevelBlur: any;
  handleToggleBypassPermissions: any;
  autoUpdate: string;
  autoUpdateOptions: any[];
  handleSelectAutoUpdate: (value: string | number) => void;
  claudeModel: string;
  claudeModelOptions: any[];
  handleSelectClaudeModel: (value: string | number) => void;
  toolSearch: boolean;
  handleToggleToolSearch: any;
  hudStatus: any;
  hudInstallAction: any;
  hudPrimaryAction: any;
  hudToggle: any;
  hc: any;
  hudLayoutOptions: any[];
  handleSelectHudLayout: any;
  handleToggleHudSeparators: any;
  hudPathLevelOptions: any[];
  handleSelectHudPathLevel: any;
  hudContextValueOptions: any[];
  handleSelectHudContextValue: any;
  hudResolvedGitStatusOptions: any[];
  handleToggleHudGitStatus: any;
  hudResolvedDisplayOptions: any[];
  handleToggleHudDisplay: any;
  hello2ccStatus: any;
  hello2ccInstallAction: any;
  hello2ccPrimaryAction: any;
  hello2ccSecondaryAction: any;
  hello2ccToggle: any;
  hello2ccSelectFields: Hello2ccConfigField[];
  handleChangeHello2ccSelect: any;
  hello2ccDraft: any;
  handleToggleHello2ccMirrorSessionModel: any;
  hello2ccHasChanges: boolean;
  setHello2ccConfigMutation: any;
  handleResetHello2ccDraft: any;
  handleSaveHello2ccConfigClick: any;
}

export default function ClaudeTab(props: ClaudeTabProps) {
  const {
    uiText,
    perm,
    permLevel,
    permDescription,
    permLevelOptions,
    handleSelectPermLevel,
    handleChangePermLevelRange,
    handleCommitPermLevelPointerUp,
    handleCommitPermLevelKeyUp,
    handleCommitPermLevelBlur,
    handleToggleBypassPermissions,
    autoUpdate,
    autoUpdateOptions,
    handleSelectAutoUpdate,
    claudeModel,
    claudeModelOptions,
    handleSelectClaudeModel,
    toolSearch,
    handleToggleToolSearch,
    hudStatus,
    hudInstallAction,
    hudPrimaryAction,
    hudToggle,
    hc,
    hudLayoutOptions,
    handleSelectHudLayout,
    handleToggleHudSeparators,
    hudPathLevelOptions,
    handleSelectHudPathLevel,
    hudContextValueOptions,
    handleSelectHudContextValue,
    hudResolvedGitStatusOptions,
    handleToggleHudGitStatus,
    hudResolvedDisplayOptions,
    handleToggleHudDisplay,
    hello2ccStatus,
    hello2ccInstallAction,
    hello2ccPrimaryAction,
    hello2ccSecondaryAction,
    hello2ccToggle,
    hello2ccSelectFields,
    handleChangeHello2ccSelect,
    hello2ccDraft,
    handleToggleHello2ccMirrorSessionModel,
    hello2ccHasChanges,
    setHello2ccConfigMutation,
    handleResetHello2ccDraft,
    handleSaveHello2ccConfigClick,
  } = props;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      {/* Permission Slider */}
      <ToolsPermissionCard
        title={uiText("权限模式", "Permission Mode", "権限モード")}
        currentLabel={uiText(perm.label_zh, perm.label_en, perm.label_ja)}
        currentDescription={permDescription}
        currentColor={perm.color}
        value={permLevel}
        options={permLevelOptions}
        onSelect={handleSelectPermLevel}
        onRangeChange={handleChangePermLevelRange}
        onRangePointerUp={handleCommitPermLevelPointerUp}
        onRangeKeyUp={handleCommitPermLevelKeyUp}
        onRangeBlur={handleCommitPermLevelBlur}
      />

      {/* Bypass Permissions */}
      <ToolsToggleCard
        title={uiText("绕过权限确认", "Bypass Permissions", "権限確認をバイパス")}
        description={uiText(
          "跳过所有权限确认，全自动执行",
          "Skip all permission prompts, fully autonomous",
          "すべての権限確認をスキップして完全自動で実行します",
        )}
        value={permLevel === 3}
        onChange={handleToggleBypassPermissions}
        labelOn="ON"
        labelOff="OFF"
      />

      {/* Auto Update */}
      <ToolsChoiceCard
        title={uiText("自动更新", "Auto Update", "自動更新")}
        description={uiText("Claude Code 更新频道", "Update channel", "Claude Code の更新チャンネル")}
        value={autoUpdate}
        onSelect={handleSelectAutoUpdate}
        options={autoUpdateOptions}
      />

      {/* Model Selection */}
      <ToolsChoiceCard
        title={uiText("模型选择", "Model", "モデル")}
        description={uiText("切换默认使用的模型", "Switch default model", "既定モデルを切り替えます")}
        value={claudeModelOptions.find((option) => claudeModel.includes(String(option.value)))?.value ?? ""}
        onSelect={handleSelectClaudeModel}
        options={claudeModelOptions}
      />

      {/* Tool Search */}
      <ToolsToggleCard
        title="Tool Search"
        description={uiText(
          "启用工具搜索功能（实验性）",
          "Enable tool search (experimental)",
          "ツール検索機能を有効化します（実験的）",
        )}
        value={toolSearch}
        onChange={handleToggleToolSearch}
        labelOn={uiText("已启用", "Enabled", "有効")}
        labelOff={uiText("已关闭", "Disabled", "無効")}
      />

      {/* StatusLine (claude-hud) */}
      <div className="card" style={{ padding: "16px 18px" }}>
        <ToolsManagedSectionHeader
          title="StatusLine (claude-hud)"
          description={uiText(
            "终端底部实时状态栏",
            "Real-time status bar at terminal bottom",
            "ターミナル下部のリアルタイムステータスバー",
          )}
          version={hudStatus?.version}
          installed={hudStatus?.installed ?? false}
          installAction={hudInstallAction}
          primaryAction={hudPrimaryAction}
          toggle={hudToggle}
        />

        {hudStatus?.installed && (
          <div
            style={{
              borderTop: "1px solid var(--border)",
              paddingTop: 12,
              display: "flex",
              flexDirection: "column",
              gap: 14,
            }}
          >
            {/* Layout */}
            <ToolsChoiceRow
              title={uiText("布局模式", "Layout Mode", "レイアウトモード")}
              value={hc.lineLayout || "expanded"}
              onSelect={handleSelectHudLayout}
              options={hudLayoutOptions}
            />

            {/* Separators */}
            <ToolsCheckboxRow
              title={uiText("分隔线", "Separators", "区切り線")}
              label={uiText(
                "活动区域前显示分隔线",
                "Show separator before activity",
                "アクティビティの前に区切り線を表示",
              )}
              checked={hc.showSeparators === true}
              onChange={handleToggleHudSeparators}
            />

            {/* Path Levels */}
            <ToolsChoiceRow
              title={uiText("路径层级", "Path Levels", "パス階層")}
              value={hc.pathLevels || 1}
              onSelect={handleSelectHudPathLevel}
              options={hudPathLevelOptions}
            />

            {/* Context Value Format */}
            <ToolsChoiceRow
              title={uiText("上下文格式", "Context Format", "コンテキスト形式")}
              value={hc.display?.contextValue || "percent"}
              onSelect={handleSelectHudContextValue}
              options={hudContextValueOptions}
            />

            {/* Git Status */}
            <ToolsCheckboxSection
              title="Git Status"
              options={hudResolvedGitStatusOptions}
              onToggle={handleToggleHudGitStatus}
            />

            {/* Display Options */}
            <ToolsCheckboxSection
              title={uiText("显示选项", "Display", "表示項目")}
              options={hudResolvedDisplayOptions}
              onToggle={handleToggleHudDisplay}
            />
          </div>
        )}
      </div>

      {/* hello2cc */}
      <div className="card" style={{ padding: "16px 18px" }}>
        <ToolsManagedSectionHeader
          title="hello2cc"
          description={uiText(
            "让第三方模型更接近 Claude Code 原生工作流",
            "Make third-party models behave more like native Claude Code",
            "サードパーティーモデルを Claude Code ネイティブに近づけます",
          )}
          version={hello2ccStatus?.version}
          installed={hello2ccStatus?.installed ?? false}
          installAction={hello2ccInstallAction}
          primaryAction={hello2ccPrimaryAction}
          secondaryAction={hello2ccSecondaryAction}
          toggle={hello2ccToggle}
          actionsWrap
        />

        {hello2ccStatus?.installed && (
          <Hello2ccConfigSection
            pathLabel={uiText("插件缓存目录", "Plugin cache path", "プラグインキャッシュパス")}
            installPath={hello2ccStatus.installPath}
            fields={hello2ccSelectFields}
            onSelectChange={handleChangeHello2ccSelect}
            mirrorTitle={uiText("镜像当前会话模型", "Mirror Session Model", "現在のセッションモデルをミラー")}
            mirrorDescription={uiText(
              "缺少显式模型时优先跟随当前会话模型槽位",
              "Prefer the current session model when no explicit slot is set",
              "明示的なモデルがない場合は現在のセッションモデルを優先します",
            )}
            mirrorValue={hello2ccDraft.mirror_session_model}
            onMirrorChange={handleToggleHello2ccMirrorSessionModel}
            mirrorLabelOn={uiText("已启用", "Enabled", "有効")}
            mirrorLabelOff={uiText("已关闭", "Disabled", "無効")}
            resetLabel={uiText("重置", "Reset", "リセット")}
            saveLabel={uiText("保存配置", "Save Config", "設定を保存")}
            hasChanges={hello2ccHasChanges}
            isSaving={setHello2ccConfigMutation.isPending}
            onReset={handleResetHello2ccDraft}
            onSave={handleSaveHello2ccConfigClick}
          />
        )}
      </div>

      {/* Proxy Advanced — 代理增强（仅 Claude） */}
      <div className="card" style={{ padding: "16px 18px" }}>
        <Suspense
          fallback={
            <div style={{ minHeight: 120, display: "flex", alignItems: "center", justifyContent: "center" }}>
              <div className="spinner" />
            </div>
          }
        >
          <ProxyAdvancedPanel embedded mode="claude" />
        </Suspense>
      </div>
    </div>
  );
}
