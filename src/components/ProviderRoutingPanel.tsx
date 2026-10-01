import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, Plus, RefreshCw, Route, Save, Trash2, Undo2 } from "lucide-react";
import { getLocale } from "../lib/i18n";
import {
  removeRoutingGroup,
  type RoutingDocument,
  type RoutingPolicy,
  type RoutingPreview,
  type RoutingProfile,
  type RoutingTool,
} from "../lib/providerRouting";
import { showToast } from "./Toast";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { SimpleSelect } from "./ui/simple-select";
import { Switch } from "./ui/switch";
import GroupEditor from "./ProviderRouting/GroupEditor";
import RuleEditor from "./ProviderRouting/RuleEditor";

const TOOLS = [
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
  { value: "gemini", label: "Gemini" },
  { value: "grokbuild", label: "Grok Build" },
  { value: "opencode", label: "OpenCode" },
  { value: "openclaw", label: "OpenClaw" },
  { value: "hermes", label: "Hermes" },
];

export default function ProviderRoutingPanel({ appType }: { appType?: RoutingTool }) {
  const locale = getLocale();
  const text = useCallback((zh: string, en: string) => (locale === "zh" ? zh : en), [locale]);
  const [tool, setTool] = useState<RoutingTool>(appType ?? "claude");
  const [document, setDocument] = useState<RoutingDocument | null>(null);
  const [policy, setPolicy] = useState<RoutingPolicy | null>(null);
  const [profiles, setProfiles] = useState<RoutingProfile[]>([]);
  const [selectedGroup, setSelectedGroup] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [previewModel, setPreviewModel] = useState("");
  const [previewBytes, setPreviewBytes] = useState(0);
  const [previewImages, setPreviewImages] = useState(false);
  const [previewThinking, setPreviewThinking] = useState(false);
  const [preview, setPreview] = useState<RoutingPreview | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const generation = useRef(0);
  const mounted = useRef(true);
  const dirty = !!policy && !!document && JSON.stringify(policy) !== JSON.stringify(document.policy);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current += 1;
    };
  }, []);
  const load = useCallback(async () => {
    const owned = ++generation.current;
    setLoading(true);
    setError("");
    setPreview(null);
    try {
      const [next, available] = await Promise.all([
        invoke<RoutingDocument>("get_provider_routing", { appType: tool }),
        invoke<RoutingProfile[]>("get_available_providers_for_failover", { appType: tool }),
      ]);
      if (!mounted.current || generation.current !== owned) return;
      if (!next?.policy || !Array.isArray(next.policy.groups) || !Array.isArray(available))
        throw new Error("Invalid routing data");
      setDocument(next);
      setPolicy(next.policy);
      setProfiles(available);
      setSelectedGroup(next.policy.groups[0]?.id ?? "");
    } catch (error) {
      if (mounted.current && generation.current === owned) setError(String(error));
    } finally {
      if (mounted.current && generation.current === owned) setLoading(false);
    }
  }, [tool]);
  useEffect(() => {
    setDocument(null);
    setPolicy(null);
    void load();
  }, [load]);

  const change = (next: RoutingPolicy) => {
    generation.current += 1;
    setPreviewing(false);
    setPolicy(next);
    setPreview(null);
    setError("");
  };
  const save = async () => {
    if (!policy || !document || saving) return;
    setSaving(true);
    setError("");
    try {
      const next = await invoke<RoutingDocument>("set_provider_routing", {
        appType: tool,
        expectedRevision: document.revision,
        policy,
      });
      if (!mounted.current) return;
      setDocument(next);
      setPolicy(next.policy);
      showToast("success", text("路由设置已保存", "Routing settings saved"));
    } catch (error) {
      if (mounted.current) setError(String(error));
    } finally {
      if (mounted.current) setSaving(false);
    }
  };
  const runPreview = async () => {
    if (!policy || previewing) return;
    setPreviewing(true);
    setError("");
    const owned = generation.current;
    try {
      const request = {
        model: previewModel,
        thinking: { type: previewThinking ? "adaptive" : "disabled" },
        messages: [
          {
            role: "user",
            content: previewImages
              ? [{ type: "image", source: { type: "base64", media_type: "image/png", data: "" } }]
              : "preview",
          },
        ],
      };
      const result = await invoke<RoutingPreview>("preview_provider_routing", {
        appType: tool,
        policy,
        request,
        relativePath: "v1/messages",
        requestBytes: previewBytes || null,
      });
      if (mounted.current && generation.current === owned) setPreview(result);
    } catch (error) {
      if (mounted.current && generation.current === owned) setError(String(error));
    } finally {
      if (mounted.current && generation.current === owned) setPreviewing(false);
    }
  };
  const group = policy?.groups.find((group) => group.id === selectedGroup);
  const busy = loading || saving;

  return (
    <section className="space-y-4 border-t border-border pt-4" aria-label={text("高级路由", "Advanced routing")}>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="flex items-center gap-2 text-sm font-semibold">
            <Route size={16} />
            {text("高级路由", "Advanced routing")}
          </h2>
          <p className="mt-1 text-xs text-muted-foreground">
            {text(
              "按请求选择分组，保留当前配置。关闭后恢复当前配置与原有故障切换顺序。",
              "Choose groups per request while retaining the active profile. Disable to restore the existing failover order.",
            )}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <SimpleSelect
            ariaLabel={text("路由工具", "Routing tool")}
            value={tool}
            disabled={busy || dirty || !!appType || previewing}
            options={TOOLS}
            onValueChange={(value) => setTool(value as RoutingTool)}
            className="w-36"
          />
          <Button
            variant="ghost"
            size="icon"
            disabled={busy || dirty || previewing}
            aria-label={text("重新读取路由", "Reload routing")}
            onClick={() => void load()}
          >
            <RefreshCw size={14} />
          </Button>
        </div>
      </div>
      {loading && (
        <p className="flex items-center gap-2 text-xs text-muted-foreground" role="status">
          <Loader2 className="animate-spin" size={14} />
          {text("正在读取路由设置…", "Loading routing settings…")}
        </p>
      )}
      {error && (
        <div role="alert" className="space-y-2 rounded-lg border border-border bg-secondary/40 p-3 text-xs">
          <p className="break-words">{error}</p>
          {!policy && (
            <Button variant="outline" disabled={busy} onClick={() => void load()}>
              {text("重试", "Retry")}
            </Button>
          )}
          {dirty && (
            <p className="text-muted-foreground">
              {text(
                "未保存修改仍保留。若设置已在别处更改，请撤销修改后重新读取。",
                "Your draft is retained. If settings changed elsewhere, undo the draft and reload.",
              )}
            </p>
          )}
        </div>
      )}
      {policy && !loading && (
        <>
          <label className="flex items-center justify-between gap-3 text-xs">
            <span>{text("启用高级路由", "Enable advanced routing")}</span>
            <Switch
              aria-label={text("启用高级路由", "Enable advanced routing")}
              checked={policy.enabled}
              disabled={busy}
              onCheckedChange={(enabled) => change({ ...policy, enabled })}
            />
          </label>
          <details open={policy.enabled || dirty} className="space-y-4">
            <summary className="cursor-pointer text-xs font-medium">
              {text("管理分组与规则", "Manage groups and rules")}
            </summary>
            <div className="space-y-2">
              <label className="flex items-center justify-between gap-3 text-xs">
                <span>{text("跳过额度已耗尽的账号", "Skip accounts with exhausted quota")}</span>
                <Switch
                  aria-label={text("跳过额度已耗尽的账号", "Skip accounts with exhausted quota")}
                  checked={policy.quotaAware ?? false}
                  disabled={busy}
                  onCheckedChange={(quotaAware) => change({ ...policy, quotaAware })}
                />
              </label>
              <p className="text-xs text-muted-foreground">
                {text(
                  "先在账号设置刷新 Codex 或 Copilot 用量。启用高级路由后，仅跳过 5 分钟内已确认额度耗尽的账号；Copilot 免费模型不受高级请求额度影响。未知或过期用量仍可尝试，固定配置不会改选其他成员。",
                  "Refresh Codex or Copilot usage in account settings first. Advanced routing skips only quota confirmed exhausted within 5 minutes; Copilot free models are unaffected by premium quota. Unknown or stale usage remains eligible, and fixed selections never choose another member.",
                )}
              </p>
            </div>
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground">{text("会话路由", "Conversation routing")}</p>
              <SimpleSelect
                ariaLabel={text("会话路由", "Conversation routing")}
                value={policy.affinity ?? "off"}
                disabled={busy}
                options={[
                  { value: "off", label: text("不固定 · 每次按分组顺序", "Off · Follow group order") },
                  {
                    value: "auto",
                    label: text("自动 · 保留工具轮次与近期缓存", "Auto · Keep tool turns and recent cache"),
                  },
                  {
                    value: "session",
                    label: text("整个会话 · 优先使用上次成功的配置", "Session · Prefer the last successful profile"),
                  },
                  {
                    value: "turn",
                    label: text("当前轮次 · 工具调用结束后重新选择", "Turn · Choose again after tool calls"),
                  },
                ]}
                onValueChange={(affinity) => change({ ...policy, affinity: affinity as RoutingPolicy["affinity"] })}
              />
              <p className="text-xs text-muted-foreground">
                {text(
                  "仅关联客户端明确标识的会话。配置不可用时继续故障切换，账号登录或配置变化会使旧绑定失效。",
                  "Only explicitly identified client conversations are linked. Unavailable profiles can fail over; login or configuration changes invalidate old bindings.",
                )}
              </p>
            </div>
            {profiles.length === 0 && (
              <p className="text-xs text-muted-foreground">
                {text(
                  "请先为此工具添加配置，然后建立路由分组。",
                  "Add a profile for this tool before creating groups.",
                )}
              </p>
            )}
            <div className="flex flex-wrap items-center gap-2">
              <SimpleSelect
                ariaLabel={text("编辑路由分组", "Edit routing group")}
                value={selectedGroup}
                disabled={busy || policy.groups.length === 0}
                options={
                  policy.groups.length
                    ? policy.groups.map((group) => ({
                        value: group.id,
                        label: group.name || text("未命名分组", "Unnamed group"),
                      }))
                    : [{ value: "", label: text("还没有分组", "No groups yet") }]
                }
                onValueChange={setSelectedGroup}
                className="min-w-0 flex-1"
              />
              <Button
                variant="outline"
                disabled={busy || profiles.length === 0 || policy.groups.length >= 64}
                onClick={() => {
                  const id = crypto.randomUUID();
                  change({
                    ...policy,
                    groups: [
                      ...policy.groups,
                      {
                        id,
                        name: text("新分组", "New group"),
                        mode: "ordered",
                        members: [{ kind: "profile", profileId: profiles[0].providerId }],
                        pickedProfileId: null,
                      },
                    ],
                    defaultGroupId: policy.defaultGroupId ?? id,
                  });
                  setSelectedGroup(id);
                }}
              >
                <Plus size={14} />
                {text("新建分组", "New group")}
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label={text("删除路由分组", "Delete routing group")}
                disabled={busy || !group}
                onClick={() => {
                  if (!group) return;
                  try {
                    const next = removeRoutingGroup(policy, group.id);
                    change(next);
                    setSelectedGroup(next.groups[0]?.id ?? "");
                  } catch {
                    setError(
                      text(
                        "此分组被其他分组引用，请先移除引用。",
                        "This group is referenced by another group. Remove that reference first.",
                      ),
                    );
                  }
                }}
              >
                <Trash2 size={14} />
              </Button>
            </div>
            {group && (
              <GroupEditor
                key={group.id}
                group={group}
                policy={policy}
                profiles={profiles}
                disabled={busy}
                onChange={change}
                text={text}
              />
            )}
            <RuleEditor policy={policy} disabled={busy} onChange={change} text={text} />
          </details>
          <details className="rounded-xl border border-border p-3">
            <summary className="cursor-pointer text-xs font-medium">
              {text("预览规则与初始顺序", "Preview rules and initial order")}
            </summary>
            <div className="mt-3 space-y-3">
              <Input
                aria-label={text("预览请求模型", "Preview request model")}
                placeholder={text("输入请求模型名称", "Enter a request model")}
                value={previewModel}
                disabled={busy || previewing}
                onChange={(event) => {
                  setPreviewModel(event.target.value);
                  setPreview(null);
                }}
              />
              <label className="block space-y-1.5 text-xs text-muted-foreground">
                <span>{text("模拟请求大小（字节，0 表示自动）", "Simulated request bytes (0 for automatic)")}</span>
                <Input
                  aria-label={text("预览请求大小", "Preview request bytes")}
                  type="number"
                  min={0}
                  max={67108864}
                  value={previewBytes}
                  disabled={busy || previewing}
                  onChange={(event) => {
                    setPreviewBytes(Math.max(0, Math.min(67108864, Math.trunc(Number(event.target.value) || 0))));
                    setPreview(null);
                  }}
                />
              </label>
              <div className="flex flex-wrap items-center gap-4">
                <label className="flex items-center gap-2 text-xs">
                  <Switch
                    checked={previewImages}
                    disabled={busy || previewing}
                    onCheckedChange={(value) => {
                      setPreviewImages(value);
                      setPreview(null);
                    }}
                  />
                  {text("包含图片", "Contains images")}
                </label>
                <label className="flex items-center gap-2 text-xs">
                  <Switch
                    checked={previewThinking}
                    disabled={busy || previewing}
                    onCheckedChange={(value) => {
                      setPreviewThinking(value);
                      setPreview(null);
                    }}
                  />
                  {text("启用推理", "Reasoning enabled")}
                </label>
                <Button variant="outline" disabled={busy || previewing} onClick={() => void runPreview()}>
                  {previewing && <Loader2 size={14} className="animate-spin" />}
                  {text("预览路由", "Preview routing")}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                {text(
                  "仅本地预览，不发送模型请求、不保存修改、不占用轮询名额。轮询分组展示初始顺序。",
                  "Local preview only: no model request, save or rotation advance. Rotating groups show their initial order.",
                )}
              </p>
              {policy.quotaAware && (
                <p className="text-xs text-muted-foreground">
                  {text(
                    "此处展示规则的候选顺序。额度筛选会在实际请求时结合账号、最终模型与最新查询结果执行。",
                    "This shows rule candidate order. Quota filtering runs on actual requests using the account, final model and latest usage observation.",
                  )}
                </p>
              )}
              {preview && (
                <div role="status" className="space-y-1 text-xs">
                  <p>
                    {preview.ruleId
                      ? `${text("匹配规则", "Matched rule")} · ${policy.rules.find((rule) => rule.id === preview.ruleId)?.name ?? preview.ruleId}`
                      : preview.groupId
                        ? text("使用默认分组", "Using default group")
                        : text("使用当前配置与故障切换队列", "Using active profile and failover queue")}
                  </p>
                  <ol className="space-y-1">
                    {preview.profileIds.map((id, index) => (
                      <li key={id} className="break-words">
                        {index + 1}.{" "}
                        {profiles.find((profile) => profile.providerId === id)?.providerName ??
                          text("配置已删除", "Deleted profile")}
                      </li>
                    ))}
                  </ol>
                </div>
              )}
            </div>
          </details>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <p className="text-xs text-muted-foreground">
              {dirty
                ? text(
                    "有未保存修改；保存或撤销后可切换工具。",
                    "Unsaved changes. Save or undo before switching tools.",
                  )
                : text("设置已与本机同步", "Settings match the saved configuration")}
            </p>
            <div className="flex items-center gap-2">
              <Button
                variant="ghost"
                disabled={busy || !dirty}
                onClick={() => {
                  if (document) {
                    change(document.policy);
                    setSelectedGroup(document.policy.groups[0]?.id ?? "");
                  }
                }}
              >
                <Undo2 size={14} />
                {text("撤销修改", "Undo changes")}
              </Button>
              <Button disabled={busy || !dirty} onClick={() => void save()}>
                {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
                {text("保存路由", "Save routing")}
              </Button>
            </div>
          </div>
        </>
      )}
    </section>
  );
}
