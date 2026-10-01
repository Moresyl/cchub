import { useState } from "react";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import type { RoutingGroup, RoutingMember, RoutingPolicy, RoutingProfile } from "../../lib/providerRouting";
import { moveRoutingItem } from "../../lib/providerRouting";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { SimpleSelect } from "../ui/simple-select";
import { Switch } from "../ui/switch";

interface Props {
  group: RoutingGroup;
  policy: RoutingPolicy;
  profiles: RoutingProfile[];
  disabled: boolean;
  onChange: (policy: RoutingPolicy) => void;
  text: (zh: string, en: string) => string;
}

export default function GroupEditor({ group, policy, profiles, disabled, onChange, text }: Props) {
  const [member, setMember] = useState("");
  const update = (patch: Partial<RoutingGroup>) =>
    onChange({ ...policy, groups: policy.groups.map((item) => (item.id === group.id ? { ...item, ...patch } : item)) });
  const profileName = (id: string) =>
    profiles.find((profile) => profile.providerId === id)?.providerName ?? text("配置已删除", "Deleted profile");
  const directProfiles = group.members.flatMap((member) =>
    member.kind === "profile" ? [{ value: member.profileId, label: profileName(member.profileId) }] : [],
  );
  const memberOptions = [
    { value: "", label: text("选择配置或子分组", "Choose a profile or subgroup") },
    ...profiles
      .filter(
        (profile) =>
          !group.members.some((member) => member.kind === "profile" && member.profileId === profile.providerId),
      )
      .map((profile) => ({ value: `profile:${profile.providerId}`, label: profile.providerName })),
    ...policy.groups
      .filter(
        (item) =>
          item.id !== group.id &&
          !group.members.some((member) => member.kind === "group" && member.groupId === item.id),
      )
      .map((item) => ({ value: `group:${item.id}`, label: `${text("分组", "Group")} · ${item.name}` })),
  ];
  const addMember = () => {
    if (!memberOptions.some((option) => option.value === member) || !member) return;
    const next: RoutingMember = member.startsWith("profile:")
      ? { kind: "profile", profileId: member.slice(8) }
      : { kind: "group", groupId: member.slice(6) };
    update({ members: [...group.members, next] });
    setMember("");
  };
  return (
    <div className="space-y-4 rounded-xl border border-border bg-card p-4">
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="space-y-1.5 text-xs text-muted-foreground">
          <span>{text("分组名称", "Group name")}</span>
          <Input
            aria-label={text("分组名称", "Group name")}
            value={group.name}
            maxLength={128}
            disabled={disabled}
            onChange={(event) => update({ name: event.target.value })}
          />
        </label>
        <label className="space-y-1.5 text-xs text-muted-foreground">
          <span>{text("选择策略", "Selection strategy")}</span>
          <SimpleSelect
            ariaLabel={text("分组选择策略", "Group strategy")}
            value={group.mode}
            disabled={disabled}
            options={[
              { value: "ordered", label: text("按顺序尝试", "Ordered failover") },
              { value: "roundRobin", label: text("轮流优先", "Round robin") },
              { value: "manual", label: text("固定选择", "Manual selection") },
            ]}
            onValueChange={(mode) =>
              update({
                mode: mode as RoutingGroup["mode"],
                pickedProfileId:
                  mode === "manual"
                    ? (group.pickedProfileId ?? directProfiles[0]?.value ?? null)
                    : group.pickedProfileId,
              })
            }
          />
        </label>
      </div>
      <label className="flex items-center justify-between gap-3 text-xs">
        <span>{text("作为默认分组", "Use as the default group")}</span>
        <Switch
          checked={policy.defaultGroupId === group.id}
          disabled={disabled}
          onCheckedChange={(checked) => onChange({ ...policy, defaultGroupId: checked ? group.id : null })}
        />
      </label>
      <p className="text-xs text-muted-foreground">
        {group.mode === "manual"
          ? text(
              "固定选择只向选中的配置发送请求，不会尝试其他分组成员。",
              "Manual selection sends only to the chosen profile, without trying other group members.",
            )
          : text(
              "前面的配置优先；轮流优先会在每次请求时改变起点。失败后继续尝试组内成员，仍遵守重试上限和熔断状态。",
              "Members retain their failover order; round robin changes the starting point on each request. Retry limits and circuit state still apply.",
            )}
      </p>
      {group.mode === "manual" && (
        <SimpleSelect
          ariaLabel={text("固定配置", "Selected profile")}
          value={group.pickedProfileId ?? ""}
          disabled={disabled}
          options={[{ value: "", label: text("请选择配置", "Choose a profile") }, ...directProfiles]}
          onValueChange={(pickedProfileId) => update({ pickedProfileId: pickedProfileId || null })}
        />
      )}
      <ol className="space-y-1.5">
        {group.members.map((item, index) => (
          <li
            key={item.kind === "profile" ? `p:${item.profileId}` : `g:${item.groupId}`}
            className="flex min-w-0 items-center gap-2 rounded-lg bg-secondary/40 px-2 py-1"
          >
            <span className="w-4 shrink-0 text-xs text-muted-foreground">{index + 1}</span>
            <span
              className="min-w-0 flex-1 truncate text-xs"
              title={
                item.kind === "profile"
                  ? profileName(item.profileId)
                  : policy.groups.find((group) => group.id === item.groupId)?.name
              }
            >
              {item.kind === "profile"
                ? profileName(item.profileId)
                : `${text("子分组", "Subgroup")} · ${policy.groups.find((group) => group.id === item.groupId)?.name ?? text("已删除", "Deleted")}`}
            </span>
            <Button
              variant="ghost"
              size="icon"
              aria-label={text(`上移成员 ${index + 1}`, `Move member ${index + 1} up`)}
              disabled={disabled || index === 0}
              onClick={() => update({ members: moveRoutingItem(group.members, index, -1) })}
            >
              <ArrowUp size={14} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label={text(`下移成员 ${index + 1}`, `Move member ${index + 1} down`)}
              disabled={disabled || index === group.members.length - 1}
              onClick={() => update({ members: moveRoutingItem(group.members, index, 1) })}
            >
              <ArrowDown size={14} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label={text(`移除成员 ${index + 1}`, `Remove member ${index + 1}`)}
              disabled={disabled}
              onClick={() =>
                update({
                  members: group.members.filter((_, position) => position !== index),
                  pickedProfileId:
                    item.kind === "profile" && group.pickedProfileId === item.profileId ? null : group.pickedProfileId,
                })
              }
            >
              <Trash2 size={14} />
            </Button>
          </li>
        ))}
      </ol>
      {group.members.length === 0 && (
        <p className="text-xs text-muted-foreground" role="status">
          {text("请添加至少一个配置或子分组。", "Add at least one profile or subgroup.")}
        </p>
      )}
      <div className="flex items-center gap-2">
        <SimpleSelect
          ariaLabel={text("添加分组成员", "Add group member")}
          value={member}
          disabled={disabled || group.members.length >= 128}
          options={memberOptions}
          onValueChange={setMember}
          className="min-w-0 flex-1"
        />
        <Button variant="outline" disabled={disabled || !member || group.members.length >= 128} onClick={addMember}>
          <Plus size={14} />
          {text("添加成员", "Add member")}
        </Button>
      </div>
    </div>
  );
}
