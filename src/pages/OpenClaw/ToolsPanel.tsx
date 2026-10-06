import { useId, useState } from "react";
import { Plus, X } from "lucide-react";
import { t } from "../../lib/i18n";
import { useSetOpenClawToolsMutation } from "../../hooks/mutations";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { SimpleSelect } from "../../components/ui/simple-select";
import ConfigPanel from "./ConfigPanel";
import useConfigDraft from "./useConfigDraft";

interface ToolsConfig {
  profile: string | null;
  allow: string[];
  deny: string[];
  [key: string]: unknown;
}
function normalize(data: unknown): ToolsConfig {
  const config = data as ToolsConfig;
  return { ...config, profile: config.profile ?? null, allow: config.allow ?? [], deny: config.deny ?? [] };
}
const profiles = ["minimal", "coding", "messaging", "full"];

export default function ToolsPanel({ blocked }: { blocked: boolean }) {
  const i = t().openClaw;
  const id = useId();
  const state = useConfigDraft("get_openclaw_tools", normalize);
  const mutation = useSetOpenClawToolsMutation();
  const config = state.draft;
  return (
    <ConfigPanel
      title={i.toolsTab}
      description={i.toolsDesc}
      ready={Boolean(config)}
      loading={state.loading}
      loadError={Boolean(state.error)}
      saveFailed={state.saveFailed}
      saving={state.saving}
      blocked={blocked}
      dirty={state.dirty}
      reload={state.reload}
      reset={state.reset}
      save={() => {
        if (config) void state.save(config, (tools) => mutation.mutateAsync({ tools }), blocked);
      }}
    >
      <div className="max-w-sm space-y-1.5">
        <label htmlFor={id} className="text-xs font-medium">
          {i.toolProfile}
        </label>
        <SimpleSelect
          id={id}
          value={config?.profile ?? ""}
          ariaLabel={i.toolProfile}
          className="w-full"
          disabled={blocked || state.saving}
          options={[
            { value: "", label: i.noProfile },
            ...[...new Set([...profiles, ...(config?.profile ? [config.profile] : [])])].map((profile) => ({
              value: profile,
              label: profile,
            })),
          ]}
          onValueChange={(profile) => config && state.update({ ...config, profile: profile || null })}
        />
      </div>
      <div className="grid min-w-0 gap-5 sm:grid-cols-2">
        <ToolList
          title={i.allowList}
          items={config?.allow ?? []}
          onChange={(allow) => config && state.update({ ...config, allow })}
        />
        <ToolList
          title={i.denyList}
          items={config?.deny ?? []}
          onChange={(deny) => config && state.update({ ...config, deny })}
        />
      </div>
    </ConfigPanel>
  );
}

function ToolList({ title, items, onChange }: { title: string; items: string[]; onChange: (items: string[]) => void }) {
  const i = t().openClaw;
  const id = useId();
  const [value, setValue] = useState("");
  const name = value.trim();
  function add() {
    if (name && !items.includes(name)) {
      onChange([...items, name]);
      setValue("");
    }
  }
  return (
    <div className="min-w-0 space-y-3 rounded-lg border border-border bg-card p-4">
      <h4 className="text-xs font-medium">{title}</h4>
      <div className="flex min-h-8 flex-wrap gap-2">
        {items.length === 0 && <p className="text-xs text-[var(--text-muted)]">{i.emptyList}</p>}
        {items.map((item, index) => (
          <span
            key={`${item}-${index}`}
            className="inline-flex max-w-full items-center gap-1 rounded-md border border-border px-2 py-1 text-xs"
          >
            <span className="min-w-0 break-all">{item}</span>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label={`${i.removeTool} ${title}: ${item}`}
              onClick={() => onChange(items.filter((_, position) => position !== index))}
            >
              <X size={12} aria-hidden="true" />
            </Button>
          </span>
        ))}
      </div>
      <div className="flex min-w-0 gap-2">
        <Input
          id={id}
          className="min-w-0 flex-1"
          aria-label={`${title}: ${i.toolName}`}
          placeholder={i.toolName}
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.nativeEvent.isComposing) {
              event.preventDefault();
              add();
            }
          }}
        />
        <Button
          type="button"
          variant="secondary"
          size="icon"
          aria-label={`${i.addTool}: ${title}`}
          disabled={!name || items.includes(name)}
          onClick={add}
        >
          <Plus size={14} aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
