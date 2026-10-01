import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import type { RoutingPolicy, RoutingRule } from "../../lib/providerRouting";
import { moveRoutingItem } from "../../lib/providerRouting";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { SimpleSelect } from "../ui/simple-select";
import { Switch } from "../ui/switch";

interface Props {
  policy: RoutingPolicy;
  disabled: boolean;
  onChange: (policy: RoutingPolicy) => void;
  text: (zh: string, en: string) => string;
}

export default function RuleEditor({ policy, disabled, onChange, text }: Props) {
  const update = (id: string, patch: Partial<RoutingRule>) =>
    onChange({ ...policy, rules: policy.rules.map((rule) => (rule.id === id ? { ...rule, ...patch } : rule)) });
  return (
    <div className="space-y-3 border-t border-border pt-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold">{text("条件规则", "Conditional rules")}</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            {text(
              "从上到下匹配，所有已选条件都满足时使用目标分组。未匹配时使用默认分组或当前配置。",
              "Rules run in order and require every selected condition. Otherwise the default group or active profile applies.",
            )}
          </p>
        </div>
        <Button
          variant="outline"
          disabled={disabled || policy.groups.length === 0 || policy.rules.length >= 128}
          onClick={() =>
            onChange({
              ...policy,
              rules: [
                ...policy.rules,
                {
                  id: crypto.randomUUID(),
                  name: text("新规则", "New rule"),
                  groupId: policy.groups[0].id,
                  model: "",
                  matchMode: "exact",
                  images: false,
                  thinking: false,
                  minRequestBytes: 0,
                },
              ],
            })
          }
        >
          <Plus size={14} />
          {text("添加规则", "Add rule")}
        </Button>
      </div>
      {policy.rules.map((rule, index) => (
        <div key={rule.id} className="space-y-3 rounded-xl border border-border bg-card p-3">
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">{index + 1}</span>
            <Input
              aria-label={text(`规则 ${index + 1} 名称`, `Rule ${index + 1} name`)}
              value={rule.name}
              disabled={disabled}
              onChange={(event) => update(rule.id, { name: event.target.value })}
              maxLength={128}
            />
            <Button
              variant="ghost"
              size="icon"
              disabled={disabled || index === 0}
              aria-label={text(`上移规则 ${index + 1}`, `Move rule ${index + 1} up`)}
              onClick={() => onChange({ ...policy, rules: moveRoutingItem(policy.rules, index, -1) })}
            >
              <ArrowUp size={14} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              disabled={disabled || index === policy.rules.length - 1}
              aria-label={text(`下移规则 ${index + 1}`, `Move rule ${index + 1} down`)}
              onClick={() => onChange({ ...policy, rules: moveRoutingItem(policy.rules, index, 1) })}
            >
              <ArrowDown size={14} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              disabled={disabled}
              aria-label={text(`删除规则 ${index + 1}`, `Delete rule ${index + 1}`)}
              onClick={() => onChange({ ...policy, rules: policy.rules.filter((item) => item.id !== rule.id) })}
            >
              <Trash2 size={14} />
            </Button>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="space-y-1.5 text-xs text-muted-foreground">
              <span>{text("目标分组", "Target group")}</span>
              <SimpleSelect
                ariaLabel={text(`规则 ${index + 1} 目标分组`, `Rule ${index + 1} target group`)}
                value={rule.groupId}
                disabled={disabled}
                options={policy.groups.map((group) => ({ value: group.id, label: group.name }))}
                onValueChange={(groupId) => update(rule.id, { groupId })}
              />
            </label>
            <label className="space-y-1.5 text-xs text-muted-foreground">
              <span>{text("模型匹配方式", "Model matching")}</span>
              <SimpleSelect
                ariaLabel={text(`规则 ${index + 1} 匹配方式`, `Rule ${index + 1} match mode`)}
                value={rule.matchMode}
                disabled={disabled}
                options={[
                  { value: "exact", label: text("完全相同", "Exact") },
                  { value: "prefix", label: text("以此开头", "Prefix") },
                  { value: "contains", label: text("包含文本", "Contains") },
                ]}
                onValueChange={(matchMode) => update(rule.id, { matchMode: matchMode as RoutingRule["matchMode"] })}
              />
            </label>
            <label className="space-y-1.5 text-xs text-muted-foreground">
              <span>{text("请求模型（留空表示不限）", "Request model (blank for any)")}</span>
              <Input
                aria-label={text(`规则 ${index + 1} 模型`, `Rule ${index + 1} model`)}
                value={rule.model}
                maxLength={256}
                disabled={disabled}
                onChange={(event) => update(rule.id, { model: event.target.value })}
              />
            </label>
            <label className="space-y-1.5 text-xs text-muted-foreground">
              <span>{text("最小请求大小（字节，0 表示不限）", "Minimum request bytes (0 for any)")}</span>
              <Input
                aria-label={text(`规则 ${index + 1} 最小请求大小`, `Rule ${index + 1} minimum bytes`)}
                type="number"
                min={0}
                max={67108864}
                value={rule.minRequestBytes}
                disabled={disabled}
                onChange={(event) =>
                  update(rule.id, {
                    minRequestBytes: Math.max(0, Math.min(67108864, Math.trunc(Number(event.target.value) || 0))),
                  })
                }
              />
            </label>
          </div>
          <div className="flex flex-wrap gap-4">
            <label className="flex items-center gap-2 text-xs">
              <Switch
                checked={rule.images}
                disabled={disabled}
                onCheckedChange={(images) => update(rule.id, { images })}
              />
              {text("包含图片", "Contains images")}
            </label>
            <label className="flex items-center gap-2 text-xs">
              <Switch
                checked={rule.thinking}
                disabled={disabled}
                onCheckedChange={(thinking) => update(rule.id, { thinking })}
              />
              {text("启用推理", "Reasoning enabled")}
            </label>
          </div>
          {!rule.model && !rule.images && !rule.thinking && !rule.minRequestBytes && (
            <p className="text-xs text-muted-foreground" role="status">
              {text("请至少设置一个条件。", "Set at least one condition.")}
            </p>
          )}
        </div>
      ))}
    </div>
  );
}
