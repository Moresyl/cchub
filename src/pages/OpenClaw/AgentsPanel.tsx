import { useId } from "react";
import { Plus, Trash2 } from "lucide-react";
import { t } from "../../lib/i18n";
import { useSetOpenClawAgentsDefaultsMutation } from "../../hooks/mutations";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import ConfigPanel from "./ConfigPanel";
import useConfigDraft from "./useConfigDraft";

interface Model {
  primary: string;
  fallbacks: string[];
  [key: string]: unknown;
}
interface Defaults {
  model: Model | null;
  models: Record<string, { alias?: string | null; [key: string]: unknown }> | null;
  [key: string]: unknown;
}
function normalize(data: unknown): Defaults {
  const defaults = data as Defaults;
  return {
    ...defaults,
    model: defaults.model
      ? { ...defaults.model, primary: defaults.model.primary ?? "", fallbacks: defaults.model.fallbacks ?? [] }
      : null,
    models: defaults.models ?? null,
  };
}

export default function AgentsPanel({ blocked }: { blocked: boolean }) {
  const i = t().openClaw;
  const id = useId();
  const state = useConfigDraft("get_openclaw_agents_defaults", normalize);
  const mutation = useSetOpenClawAgentsDefaultsMutation();
  const defaults = state.draft;
  const model = defaults?.model ?? { primary: "", fallbacks: [] };
  const names = [model.primary, ...model.fallbacks].map((name) => name.trim());
  const issue =
    defaults?.model && names.some((name) => !name)
      ? i.emptyModel
      : names.filter(Boolean).length !== new Set(names.filter(Boolean)).size
        ? i.duplicateModel
        : null;

  return (
    <ConfigPanel
      title={i.agentsTab}
      description={i.agentsDesc}
      ready={Boolean(defaults)}
      loading={state.loading}
      loadError={Boolean(state.error)}
      saveFailed={state.saveFailed}
      saving={state.saving}
      blocked={blocked}
      dirty={state.dirty}
      issue={issue}
      reload={state.reload}
      reset={state.reset}
      save={() => {
        if (defaults && !issue)
          void state.save(defaults, (value) => mutation.mutateAsync({ defaults: value }), blocked);
      }}
    >
      <div className="space-y-1.5">
        <label htmlFor={`${id}-primary`} className="text-xs font-medium">
          {i.primaryModel}
        </label>
        <Input
          id={`${id}-primary`}
          placeholder="provider/model"
          value={model.primary}
          onChange={(event) =>
            defaults && state.update({ ...defaults, model: { ...model, primary: event.target.value } })
          }
        />
      </div>
      <div className="space-y-3">
        <h4 className="text-xs font-medium">{i.fallbackModels}</h4>
        <p className="text-xs text-[var(--text-secondary)]">{i.fallbackDesc}</p>
        {model.fallbacks.length === 0 && <p className="text-xs text-[var(--text-muted)]">{i.emptyList}</p>}
        {model.fallbacks.map((fallback, index) => (
          <div key={index} className="flex min-w-0 items-center gap-2">
            <span className="w-5 shrink-0 text-center text-xs text-[var(--text-muted)]">{index + 1}</span>
            <Input
              className="min-w-0 flex-1"
              aria-label={`${i.fallbackModels} ${index + 1}`}
              value={fallback}
              placeholder="provider/model"
              onChange={(event) =>
                defaults &&
                state.update({
                  ...defaults,
                  model: {
                    ...model,
                    fallbacks: model.fallbacks.map((value, position) =>
                      position === index ? event.target.value : value,
                    ),
                  },
                })
              }
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={`${i.removeFallback} ${index + 1}`}
              onClick={() =>
                defaults &&
                state.update({
                  ...defaults,
                  model: { ...model, fallbacks: model.fallbacks.filter((_, position) => position !== index) },
                })
              }
            >
              <Trash2 size={14} aria-hidden="true" />
            </Button>
          </div>
        ))}
        <Button
          type="button"
          variant="secondary"
          onClick={() =>
            defaults && state.update({ ...defaults, model: { ...model, fallbacks: [...model.fallbacks, ""] } })
          }
        >
          <Plus size={14} aria-hidden="true" />
          {i.addFallback}
        </Button>
      </div>
      {defaults?.models && Object.keys(defaults.models).length > 0 && (
        <div className="space-y-3 border-t border-border pt-4">
          <h4 className="text-xs font-medium">{i.modelAliases}</h4>
          {Object.entries(defaults.models).map(([name, entry], index) => (
            <div key={name} className="grid min-w-0 gap-2 sm:grid-cols-2">
              <label
                htmlFor={`${id}-alias-${index}`}
                className="min-w-0 break-all self-center font-mono text-xs text-[var(--text-secondary)]"
              >
                {name}
              </label>
              <Input
                id={`${id}-alias-${index}`}
                aria-label={`${i.modelAliases}: ${name}`}
                placeholder={i.optionalAlias}
                value={entry.alias ?? ""}
                onChange={(event) =>
                  state.update({
                    ...defaults,
                    models: { ...defaults.models, [name]: { ...entry, alias: event.target.value || null } },
                  })
                }
              />
            </div>
          ))}
        </div>
      )}
    </ConfigPanel>
  );
}
