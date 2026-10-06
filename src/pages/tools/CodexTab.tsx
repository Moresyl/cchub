import { RefreshCw } from "lucide-react";
import ReasoningEffortSelect from "../../components/ReasoningEffortSelect";
import LoadingState from "../../components/states/LoadingState";
import ErrorState from "../../components/states/ErrorState";
import { Card } from "../../components/ui/card";
import { Button } from "../../components/ui/button";
import { SimpleSelect } from "../../components/ui/simple-select";
import { Switch } from "../../components/ui/switch";
import { useCodexSettings } from "./useCodexSettings";

type UiText = (zh: string, en: string, ja?: string) => string;

export default function CodexTab({ uiText }: { uiText: UiText }) {
  const { settings, loading, refreshing, loadError, saveError, saving, save, reload } = useCodexSettings();
  if (loading) return <LoadingState label={uiText("读取配置…", "Reading configuration…", "設定を読み込み中…")} />;
  if (loadError || !settings)
    return (
      <ErrorState
        title={uiText("无法读取 Codex 设置", "Cannot read Codex settings", "Codex 設定を読み込めません")}
        message={uiText(
          "请检查设置中配置的目录、文件权限和 TOML 格式。读取成功前不会显示默认值或修改配置。",
          "Check the configured location, file permissions and TOML format. Settings stay unavailable until the file can be read.",
          "設定先、ファイルの権限、TOML 形式を確認してください。読み込めるまで設定を変更できません。",
        )}
        retryLabel={uiText("重新读取", "Reload", "再読み込み")}
        onRetry={() => void reload()}
      />
    );
  const disabled = saving || refreshing || saveError;
  const options = [
    { value: "read-only", label: uiText("只读 · 按需审批", "Read only · On request", "読み取り専用 · 必要時承認") },
    {
      value: "workspace-write",
      label: uiText("工作区写入 · 按需审批", "Workspace write · On request", "作業領域に書き込み · 必要時承認"),
    },
    {
      value: "danger-full-access",
      label: uiText("完全访问 · 无需审批", "Full access · No approvals", "フルアクセス · 承認なし"),
    },
  ];
  if (!options.some((option) => option.value === settings.approval_mode)) {
    options.unshift({
      value: settings.approval_mode,
      label:
        settings.approval_mode === "default"
          ? uiText("使用客户端默认值", "Use client defaults", "クライアントの既定値")
          : uiText("保留自定义权限", "Keep custom permissions", "カスタム権限を保持"),
    });
  }
  const change = (key: string, value: string) => void save(key, value);
  return (
    <div className="min-w-0 space-y-3" aria-busy={saving || refreshing}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-xs text-muted-foreground" role="status">
          {saving
            ? uiText("正在保存…", "Saving…", "保存中…")
            : refreshing
              ? uiText("重新读取中…", "Reloading…", "再読み込み中…")
              : uiText(
                  "修改后自动保存到配置文件",
                  "Changes are saved to the configuration file",
                  "変更は設定ファイルに保存されます",
                )}
        </p>
        <Button variant="ghost" size="sm" disabled={saving || refreshing} onClick={() => void reload()}>
          <RefreshCw size={14} />
          {uiText("重新读取", "Reload", "再読み込み")}
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        {uiText(
          "这里管理用户配置文件中的值；项目配置、配置档案与组织策略可能覆盖这些设置。",
          "These are user configuration values. Project settings, configuration profiles and organization policies may override them.",
          "ユーザー設定ファイルの値を管理します。プロジェクト設定、設定プロファイル、組織ポリシーが優先される場合があります。",
        )}
      </p>
      {saveError && (
        <Card className="space-y-2 border-[var(--warning)] p-4" role="alert">
          <p className="text-sm font-semibold">
            {uiText("设置未保存", "Settings were not saved", "設定を保存できませんでした")}
          </p>
          <p className="text-xs text-muted-foreground">
            {uiText(
              "配置文件或目录可能已变更，或写入失败。界面仍显示上次确认的值；请重新读取后再修改。",
              "The file or its location may have changed, or writing failed. The last confirmed values remain visible; reload before editing again.",
              "ファイルや保存先が変更されたか、書き込みに失敗しました。確認済みの値を表示しています。再読み込みしてから変更してください。",
            )}
          </p>
        </Card>
      )}
      <Card className="min-w-0 space-y-3 p-4">
        <h4 className="text-sm font-semibold">{uiText("权限模式", "Permissions", "権限モード")}</h4>
        <p className="text-xs text-muted-foreground">
          {uiText(
            "同时设置沙箱范围与审批策略。完全访问允许在工作区外执行操作，且不请求审批。",
            "Sets sandbox access and approval policy together. Full access permits actions outside the workspace without approval.",
            "サンドボックス範囲と承認ポリシーを設定します。フルアクセスでは作業領域外の操作も承認なしで実行できます。",
          )}
        </p>
        <SimpleSelect
          value={settings.approval_mode}
          options={options.map((option) => ({
            ...option,
            disabled: option.value === "default" || option.value === "custom",
          }))}
          ariaLabel={uiText("权限模式", "Permissions", "権限モード")}
          disabled={disabled || settings.profile_selected}
          onValueChange={(value) => change("approval_mode", value)}
        />
        {settings.approval_mode === "custom" && (
          <p className="break-words text-xs text-muted-foreground">
            {uiText(
              "当前使用自定义策略或配置档案，保留原值。选择预设会替换活动权限策略；配置档案的覆盖规则需在配置文件中编辑。",
              "Custom policies or a configuration profile are preserved. Selecting a preset replaces the active permissions policy; profile overrides must be edited in the configuration file.",
              "カスタムポリシーや設定プロファイルを保持します。プリセット選択で有効な権限ポリシーを置き換えます。プロファイルの上書きは設定ファイルで編集してください。",
            )}
          </p>
        )}
        {settings.legacy_personality && (
          <p className="text-xs text-[var(--warning)]">
            {uiText(
              "检测到旧版误写的回复风格字段。重新选择权限模式会修正该字段。",
              "An invalid response-style value from an older version was found. Selecting a permissions mode repairs it.",
              "旧版が誤って保存した応答スタイル値を検出しました。権限モードを選び直すと修正します。",
            )}
          </p>
        )}
      </Card>
      <Card className="space-y-3 p-4">
        <h4 className="text-sm font-semibold">{uiText("推理强度", "Reasoning effort", "推論強度")}</h4>
        <fieldset disabled={disabled} className="min-w-0 border-0 p-0">
          <ReasoningEffortSelect
            value={settings.reasoning_effort}
            onValueChange={(value) => change("reasoning_effort", value)}
            localeText={uiText}
          />
        </fieldset>
      </Card>
      <Card className="flex items-start justify-between gap-4 p-4">
        <div className="min-w-0 space-y-1">
          <h4 className="text-sm font-semibold">
            {uiText("1M 上下文上限", "1M context limit", "1M コンテキスト上限")}
          </h4>
          <p className="text-xs text-muted-foreground">
            {uiText(
              "将客户端上限设为 1,000,000 token，仍需模型和服务支持。关闭仅移除此 1M 设置，保留其他自定义上限。",
              "Sets the client limit to 1,000,000 tokens; the model and service must support it. Turning it off removes only a 1M override, preserving other limits.",
              "クライアント上限を 1,000,000 トークンに設定します。モデルとサービスの対応が必要です。無効化しても他のカスタム上限は保持します。",
            )}
          </p>
          {settings.context_window !== null && (
            <p className="text-xs text-muted-foreground">
              {uiText("当前上限", "Current limit", "現在の上限")}：{settings.context_window.toLocaleString()}
            </p>
          )}
        </div>
        <Switch
          aria-label={uiText("1M 上下文上限", "1M context limit", "1M コンテキスト上限")}
          disabled={disabled}
          checked={settings.context_window_1m}
          onCheckedChange={(value) => change("context_window_1m", String(value))}
        />
      </Card>
      <Card className="flex items-start justify-between gap-4 p-4">
        <div className="min-w-0 space-y-1">
          <h4 className="text-sm font-semibold">
            {uiText("响应存储兼容设置", "Response storage compatibility", "応答保存の互換設定")}
          </h4>
          <p className="text-xs text-muted-foreground">
            {uiText(
              "保留旧配置的 disable_response_storage 字段。当前官方配置参考未列出此字段，不能据此保证本地或服务端不保存记录。",
              "Keeps the legacy disable_response_storage field. The current official reference does not list it; this cannot guarantee that local or server records are disabled.",
              "旧設定の disable_response_storage を保持します。現在の公式設定資料には記載がなく、ローカルやサーバー側の記録停止を保証するものではありません。",
            )}
          </p>
        </div>
        <Switch
          aria-label={uiText("响应存储兼容设置", "Response storage compatibility", "応答保存の互換設定")}
          disabled={disabled}
          checked={settings.disable_response_storage}
          onCheckedChange={(value) => change("disable_response_storage", String(value))}
        />
      </Card>
    </div>
  );
}
