import { RefreshCw } from "lucide-react";
import ModelSelector from "../../components/ModelSelector";
import LoadingState from "../../components/states/LoadingState";
import ErrorState from "../../components/states/ErrorState";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { SimpleSelect, type SimpleSelectOption } from "../../components/ui/simple-select";
import { useClaudeSettings } from "./useClaudeSettings";
import ToolSearchThreshold from "./ToolSearchThreshold";

type UiText = (zh: string, en: string, ja?: string) => string;
const models = ["opus", "sonnet", "haiku", "opusplan"].map((id) => ({ id, displayName: id }));
function preserve(options: SimpleSelectOption[], value: string): SimpleSelectOption[] {
  return options.some((option) => option.value === value)
    ? options
    : [{ value, label: value, disabled: true }, ...options];
}

export default function ClaudeSettingsSection({ uiText }: { uiText: UiText }) {
  const state = useClaudeSettings();
  const { settings, loading, loadError, saving, refreshing, saveError, reload, save } = state;
  if (loading) return <LoadingState />;
  if (loadError || !settings)
    return (
      <ErrorState
        title={uiText("无法读取 Claude 设置", "Unable to read Claude settings")}
        message={uiText(
          "请检查配置目录、文件权限和 JSON 格式。读取失败时不会使用默认值替代。",
          "Check the configured directory, file permissions and JSON format. Failed reads do not substitute defaults.",
        )}
        retryLabel={uiText("重新读取", "Reload")}
        onRetry={() => void reload()}
      />
    );
  const disabled = saving || refreshing || saveError;
  const defaultLabel = uiText("使用客户端默认值", "Use client defaults");
  const modes = preserve(
    [
      { value: "", label: defaultLabel },
      { value: "default", label: uiText("标准 · 按需确认", "Standard · Ask when needed") },
      { value: "acceptEdits", label: uiText("自动接受文件编辑", "Accept file edits") },
      { value: "plan", label: uiText("规划 · 批准后执行", "Plan · Execute after approval") },
      { value: "auto", label: uiText("自动 · 分类器检查", "Auto · Classifier checks") },
      { value: "dontAsk", label: uiText("不询问 · 拒绝未获准操作", "Don't ask · Deny unapproved actions") },
      { value: "bypassPermissions", label: uiText("绕过权限确认", "Bypass permissions") },
      { value: "manual", label: uiText("手动 · 标准模式别名", "Manual · Standard alias") },
    ],
    settings.permission_mode,
  );
  const updates = preserve(
    [
      { value: "", label: defaultLabel },
      { value: "latest", label: uiText("最新频道", "Latest channel") },
      { value: "stable", label: uiText("稳定频道", "Stable channel") },
      { value: "disabled", label: uiText("关闭后台自动更新", "Disable background updates") },
    ],
    settings.auto_update,
  );
  const search = preserve(
    [
      { value: "", label: defaultLabel },
      { value: "true", label: uiText("启用 · 延迟加载工具", "Enable · Defer tools") },
      { value: "auto", label: uiText("自动 · 按上下文占比", "Auto · Based on context") },
      { value: "false", label: uiText("关闭 · 提前加载全部工具", "Disable · Load all tools upfront") },
    ],
    settings.tool_search,
  );
  const change = (key: string, value: string) => void save(key, value);
  return (
    <div className="min-w-0 space-y-3" aria-busy={saving || refreshing}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p role="status" className="text-xs text-muted-foreground">
          {saving
            ? uiText("正在保存…", "Saving…")
            : refreshing
              ? uiText("重新读取中…", "Reloading…")
              : uiText("修改后自动保存到用户配置", "Changes are saved to user settings")}
        </p>
        <Button variant="ghost" size="sm" disabled={saving || refreshing} onClick={() => void reload()}>
          <RefreshCw size={14} />
          {uiText("重新读取", "Reload")}
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        {uiText(
          "显示配置目录中 settings.json 的值。项目配置、环境变量、启动参数与组织策略可能覆盖；修改通常在新会话生效。",
          "Shows settings.json in the configured directory. Project settings, environment variables, launch flags and organization policies may override it; changes usually apply to new sessions.",
        )}
      </p>
      {saveError && (
        <Card role="alert" className="space-y-2 border-[var(--warning)] p-4">
          <p className="text-sm font-semibold">{uiText("设置未保存", "Settings were not saved")}</p>
          <p className="text-xs text-muted-foreground">
            {uiText(
              "文件或目录可能已变更，或写入失败。保留上次确认值，请重新读取后再修改。",
              "The file or location may have changed, or writing failed. Confirmed values remain; reload before editing.",
            )}
          </p>
        </Card>
      )}
      <Card className="min-w-0 space-y-3 p-4">
        <h4 className="text-sm font-semibold">{uiText("权限模式", "Permissions")}</h4>
        <p className="text-xs text-muted-foreground">
          {uiText(
            "只修改默认模式，保留所有允许、询问和拒绝规则。绕过模式不询问权限，已有拒绝规则仍保留。自动模式和手动别名需要客户端版本支持。",
            "Changes only the default mode, preserving allow, ask and deny rules. Bypass mode skips prompts; existing deny rules remain. Auto mode and the manual alias require client support.",
          )}
        </p>
        <SimpleSelect
          value={settings.permission_mode}
          options={modes}
          ariaLabel={uiText("权限模式", "Permissions")}
          disabled={disabled}
          onValueChange={(value) => change("permission_mode", value)}
        />
        {settings.permission_mode === "normal" && (
          <p className="text-xs text-[var(--warning)]">
            {uiText(
              "发现旧版权限值 normal，客户端可能忽略。主动选择原生模式可修复，已有权限规则会保留。",
              "The legacy value normal may be ignored by the client. Choose a native mode to repair it; existing permission rules will remain.",
            )}
          </p>
        )}
        <p className="text-xs text-muted-foreground">
          {uiText(
            `已有规则：允许 ${settings.allow_count} · 询问 ${settings.ask_count} · 拒绝 ${settings.deny_count}`,
            `Existing rules: allow ${settings.allow_count} · ask ${settings.ask_count} · deny ${settings.deny_count}`,
          )}
        </p>
      </Card>
      <Card className="min-w-0 space-y-3 p-4">
        <h4 className="text-sm font-semibold">{uiText("自动更新", "Auto updates")}</h4>
        <p className="text-xs text-muted-foreground">
          {uiText(
            "关闭只禁用后台自动更新，仍可手动更新。系统包管理器和其他更新策略可能覆盖此设置。",
            "Disabling stops background updates; manual updates remain available. Package managers and other update policies may override this setting.",
          )}
        </p>
        <SimpleSelect
          value={settings.auto_update}
          options={updates}
          ariaLabel={uiText("自动更新", "Auto updates")}
          disabled={disabled}
          onValueChange={(value) => change("auto_update", value)}
        />
      </Card>
      <Card className="min-w-0 space-y-3 p-4">
        <h4 className="text-sm font-semibold">{uiText("默认模型", "Default model")}</h4>
        <p className="text-xs text-muted-foreground">
          {uiText(
            "选择模型别名，或搜索框中输入完整模型 ID；清除后由客户端决定。保留已有自定义模型 ID。",
            "Choose an alias or enter a full model ID in search; clear to use the client default. Existing custom IDs are preserved.",
          )}
        </p>
        <ModelSelector
          value={settings.model}
          models={models}
          label={uiText("默认模型", "Default model")}
          placeholder={defaultLabel}
          disabled={disabled}
          onChange={(value) => change("model", value)}
        />
      </Card>
      <Card className="min-w-0 space-y-3 p-4">
        <h4 className="text-sm font-semibold">Tool Search</h4>
        <p className="text-xs text-muted-foreground">
          {uiText(
            "启用需要模型和服务支持工具引用。使用代理时请确认兼容；其他环境设置可能覆盖该值。",
            "Enabling requires model and service support for tool references. Check proxy compatibility; other environment settings may override this value.",
          )}
        </p>
        <SimpleSelect
          value={settings.tool_search}
          options={search}
          ariaLabel="Tool Search"
          disabled={disabled}
          onValueChange={(value) => change("tool_search", value)}
        />
        {(settings.tool_search === "auto" || /^auto:\d+$/.test(settings.tool_search)) && (
          <ToolSearchThreshold
            key={settings.tool_search}
            value={settings.tool_search}
            disabled={disabled}
            uiText={uiText}
            onSave={(value) => change("tool_search", value)}
          />
        )}
        {settings.legacy_tool_search && (
          <p className="break-words text-xs text-[var(--warning)]">
            {uiText(
              `发现旧版 settings.local.json 值：${settings.legacy_tool_search}。它不属于全局用户设置；主动选择新值后，将写入 settings.json 并移除旧字段。`,
              `Legacy settings.local.json value: ${settings.legacy_tool_search}. It is not a global user setting. Choosing a new value writes settings.json and removes the legacy field.`,
            )}
          </p>
        )}
      </Card>
    </div>
  );
}
