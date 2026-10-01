import { useId, useState } from "react";
import { FolderOpen } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

interface Props {
  label: string;
  value: string;
  pickerLabel: string;
  busy: boolean;
  onSave: (value: string) => Promise<string | undefined>;
  onPick: () => Promise<string | null | undefined>;
}

export default function SettingsToolPathInput({ label, value, pickerLabel, busy, onSave, onPick }: Props) {
  const id = useId();
  const [draft, setDraft] = useState({ source: value, value });
  // Adopt refreshed/picked paths only when the field is pristine or the server
  // acknowledges this draft. Unrelated refreshes must not replace active edits.
  let displayed = draft.value;
  if (draft.source !== value) {
    displayed = draft.value === draft.source || draft.value.trim() === value ? value : draft.value;
    setDraft({ source: value, value: displayed });
  }

  return (
    <div className="min-w-0 space-y-2">
      <label htmlFor={id} className="text-[12px] text-muted-foreground">
        {label}
      </label>
      <div className="flex min-w-0 items-center gap-2">
        <Input
          id={id}
          className="min-w-0 flex-1 font-mono"
          value={displayed}
          disabled={busy}
          onChange={(event) => setDraft({ source: value, value: event.target.value })}
          onBlur={async () => {
            if (!busy && displayed.trim() !== value) {
              const canonical = await onSave(displayed);
              if (canonical !== undefined) setDraft({ source: value, value: canonical });
            }
          }}
        />
        <Button
          type="button"
          variant="outline"
          size="icon"
          disabled={busy}
          aria-label={pickerLabel}
          title={pickerLabel}
          // A pointer choice replaces this field explicitly; do not first start
          // an autosave on blur and race the file picker with the old draft.
          onPointerDown={(event) => {
            if (event.button === 0) event.preventDefault();
          }}
          onClick={async () => {
            const picked = await onPick();
            if (picked) setDraft({ source: value, value: picked });
          }}
        >
          <FolderOpen size={14} aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
