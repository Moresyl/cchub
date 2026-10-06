import { useId, useState } from "react";
import { Eye, FileText, Save } from "lucide-react";
import CodeEditor from "../../components/CodeEditor";
import MarkdownPreview from "../../components/MarkdownPreview";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Input } from "../../components/ui/input";
import type { PromptDraft, Text } from "./types";

interface Props {
  draft: PromptDraft;
  onChange: (draft: PromptDraft) => void;
  onSave: (activate: boolean) => void;
  onClose: () => void;
  writing: boolean;
  blocked: boolean;
  canWriteLive: boolean;
  text: Text;
}
export default function PromptEditor({
  draft,
  onChange,
  onSave,
  onClose,
  writing,
  blocked,
  canWriteLive,
  text,
}: Props) {
  const [preview, setPreview] = useState(false);
  const [nameTouched, setNameTouched] = useState(false);
  const nameLabelId = useId();
  const nameHintId = useId();
  const invalidName = !draft.name.trim() || [...draft.name.trim()].length > 120;
  const invalidContent = new TextEncoder().encode(draft.content).length > 1024 * 1024;
  return (
    <Card className="flex min-w-0 flex-col overflow-clip">
      <div className="grid items-start gap-4 border-b border-border p-4 sm:grid-cols-2">
        <label className="grid min-w-0 gap-1.5 text-xs font-medium">
          <span id={nameLabelId}>{text("名称", "Name", "名前")}</span>
          <Input
            autoFocus
            maxLength={240}
            value={draft.name}
            disabled={writing}
            aria-labelledby={nameLabelId}
            aria-invalid={(nameTouched && invalidName) || undefined}
            aria-describedby={nameHintId}
            onBlur={() => setNameTouched(true)}
            onChange={(event) => onChange({ ...draft, name: event.target.value })}
          />
          <span
            id={nameHintId}
            className={nameTouched && invalidName ? "text-[var(--danger)]" : "text-muted-foreground"}
          >
            {text("必填，最多 120 个字符", "Required, up to 120 characters", "必須、120 文字以内")}
          </span>
        </label>
        <label className="grid min-w-0 gap-1.5 text-xs font-medium">
          {text("说明（可选）", "Description (optional)", "説明（任意）")}
          <Input
            maxLength={2000}
            value={draft.description}
            disabled={writing}
            onChange={(event) => onChange({ ...draft, description: event.target.value })}
          />
        </label>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2 px-4 py-3">
        <span id="prompt-content-label" className="text-xs font-medium">
          {text("指令内容", "Instructions", "指示内容")}
        </span>
        <Button variant="ghost" onClick={() => setPreview(!preview)} aria-pressed={preview}>
          <Eye size={14} />
          {preview ? text("返回编辑", "Edit", "編集") : text("预览", "Preview", "プレビュー")}
        </Button>
      </div>
      <div className="min-w-0 px-4 pb-4" role="group" aria-labelledby="prompt-content-label">
        {preview ? (
          <div className="markdown-preview min-h-[280px] break-words">
            <MarkdownPreview content={draft.content} loadingLabel={text("正在加载预览…", "Loading preview…")} />
          </div>
        ) : (
          <CodeEditor
            value={draft.content}
            language="markdown"
            minHeight={280}
            maxHeight={480}
            readOnly={writing}
            onChange={(content) => onChange({ ...draft, content })}
          />
        )}
        {invalidContent && (
          <p role="alert" className="mt-2 text-xs text-[var(--danger)]">
            {text("内容超过 1 MiB，请缩短后再保存。", "Content exceeds 1 MiB. Shorten it before saving.")}
          </p>
        )}
      </div>
      <div className="sticky bottom-0 z-10 flex flex-wrap items-center justify-end gap-2 border-t border-border bg-card p-3">
        <Button variant="ghost" disabled={writing} onClick={onClose}>
          {text("取消", "Cancel", "キャンセル")}
        </Button>
        <Button
          variant="secondary"
          disabled={writing || blocked || invalidName || invalidContent || (draft.enabled && !canWriteLive)}
          onClick={() => onSave(false)}
        >
          <Save size={14} />
          {text("保存", "Save", "保存")}
        </Button>
        <Button
          disabled={writing || blocked || invalidName || invalidContent || !canWriteLive}
          onClick={() => onSave(true)}
        >
          <FileText size={14} />
          {text("保存并启用", "Save & activate", "保存して有効化")}
        </Button>
      </div>
    </Card>
  );
}
