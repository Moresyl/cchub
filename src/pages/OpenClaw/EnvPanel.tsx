import { useId, useRef } from "react";
import { Plus, Trash2 } from "lucide-react";
import { t } from "../../lib/i18n";
import { useSetOpenClawEnvMutation } from "../../hooks/mutations";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import CodeEditor from "../../components/CodeEditor";
import ConfigPanel from "./ConfigPanel";
import useConfigDraft from "./useConfigDraft";

interface Row {
  id: string;
  key: string;
  value: string;
}
interface EnvDraft {
  rows: Row[];
  structured: Record<string, unknown>;
}
function normalize(data: unknown): EnvDraft {
  const entries = Object.entries(data as Record<string, unknown>);
  return {
    rows: entries
      .filter(([, value]) => typeof value === "string")
      .map(([key, value], index) => ({ id: `loaded-${index}`, key, value: value as string })),
    structured: Object.fromEntries(entries.filter(([, value]) => typeof value !== "string")),
  };
}
export default function EnvPanel({ blocked }: { blocked: boolean }) {
  const i = t().openClaw;
  const id = useId();
  const nextId = useRef(0);
  const mutation = useSetOpenClawEnvMutation();
  const state = useConfigDraft("get_openclaw_env", normalize);
  const draft = state.draft;
  const rows = draft?.rows ?? [];
  const structured = draft?.structured ?? {};
  const keys = rows.map((row) => row.key.trim());
  const issue = keys.some((key) => !key)
    ? i.emptyKey
    : new Set(keys).size !== keys.length || keys.some((key) => Object.prototype.hasOwnProperty.call(structured, key))
      ? i.duplicateKey
      : null;
  function save() {
    if (!draft || issue) return;
    const env = Object.fromEntries([...Object.entries(structured), ...rows.map((row) => [row.key.trim(), row.value])]);
    void state.save(draft, () => mutation.mutateAsync({ env }), blocked);
  }
  return (
    <ConfigPanel
      title={i.envTab}
      description={i.envDesc}
      ready={Boolean(draft)}
      loading={state.loading}
      loadError={Boolean(state.error)}
      saveFailed={state.saveFailed}
      saving={state.saving}
      blocked={blocked}
      dirty={state.dirty}
      issue={issue}
      reload={state.reload}
      reset={state.reset}
      save={save}
    >
      {rows.length === 0 && (
        <p className="rounded-lg border border-dashed border-border p-5 text-center text-xs text-[var(--text-muted)]">
          {i.emptyEnv}
        </p>
      )}
      <div className="space-y-3">
        {rows.map((row, index) => (
          <div key={row.id} className="flex min-w-0 items-end gap-2 rounded-lg border border-border bg-card p-3">
            <div className="grid min-w-0 flex-1 gap-3 sm:grid-cols-[1fr_2fr]">
              <div className="min-w-0 space-y-1.5">
                <label htmlFor={`${id}-${row.id}-key`} className="text-xs text-[var(--text-secondary)]">
                  {i.variableName}
                </label>
                <Input
                  id={`${id}-${row.id}-key`}
                  value={row.key}
                  placeholder="API_KEY"
                  spellCheck={false}
                  autoComplete="off"
                  aria-label={`${i.variableName} ${index + 1}`}
                  aria-invalid={Boolean(issue)}
                  onChange={(event) =>
                    draft &&
                    state.update({
                      ...draft,
                      rows: rows.map((entry) => (entry.id === row.id ? { ...entry, key: event.target.value } : entry)),
                    })
                  }
                />
              </div>
              <div className="min-w-0 space-y-1.5">
                <label htmlFor={`${id}-${row.id}-value`} className="text-xs text-[var(--text-secondary)]">
                  {i.variableValue}
                </label>
                <Input
                  id={`${id}-${row.id}-value`}
                  value={row.value}
                  spellCheck={false}
                  autoComplete="off"
                  aria-label={`${i.variableValue} ${index + 1}`}
                  onChange={(event) =>
                    draft &&
                    state.update({
                      ...draft,
                      rows: rows.map((entry) =>
                        entry.id === row.id ? { ...entry, value: event.target.value } : entry,
                      ),
                    })
                  }
                />
              </div>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={`${i.removeVar} ${index + 1}`}
              onClick={() => draft && state.update({ ...draft, rows: rows.filter((entry) => entry.id !== row.id) })}
            >
              <Trash2 size={14} aria-hidden="true" />
            </Button>
          </div>
        ))}
      </div>
      <Button
        type="button"
        variant="secondary"
        onClick={() =>
          draft && state.update({ ...draft, rows: [...rows, { id: `new-${nextId.current++}`, key: "", value: "" }] })
        }
      >
        <Plus size={14} aria-hidden="true" />
        {i.addVar}
      </Button>
      {Object.keys(structured).length > 0 && (
        <div className="space-y-2">
          <h4 className="text-xs font-medium">{i.structuredEnv}</h4>
          <p className="text-xs text-[var(--text-secondary)]">{i.structuredEnvDesc}</p>
          <CodeEditor
            value={JSON.stringify(structured, null, 2)}
            language="json"
            readOnly
            minHeight={100}
            maxHeight={240}
            ariaLabel={i.structuredEnv}
          />
        </div>
      )}
    </ConfigPanel>
  );
}
