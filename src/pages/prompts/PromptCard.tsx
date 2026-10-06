import { FileText, Pencil, Trash2 } from "lucide-react";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import type { PromptRecord, Text } from "./types";

interface Props {
  prompt: PromptRecord;
  matchesLive: boolean;
  disabled: boolean;
  canWriteLive: boolean;
  onEdit: () => void;
  onDelete: () => void;
  onActivate: () => void;
  text: Text;
}
export default function PromptCard({
  prompt,
  matchesLive,
  disabled,
  canWriteLive,
  onEdit,
  onDelete,
  onActivate,
  text,
}: Props) {
  const updated = new Date(prompt.updatedAt);
  const validDate = Number.isFinite(updated.getTime());
  return (
    <Card className="flex min-w-0 flex-col gap-3 p-4">
      <div className="flex items-start gap-2">
        <FileText size={16} className="mt-0.5 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <h2 className="break-words text-sm font-semibold">{prompt.name}</h2>
          {prompt.description && <p className="mt-1 break-words text-xs text-muted-foreground">{prompt.description}</p>}
        </div>
        <Button
          variant="ghost"
          size="icon"
          disabled={disabled}
          onClick={onEdit}
          aria-label={text(`编辑 ${prompt.name}`, `Edit ${prompt.name}`)}
        >
          <Pencil size={14} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          disabled={disabled}
          onClick={onDelete}
          aria-label={text(`删除 ${prompt.name}`, `Delete ${prompt.name}`)}
        >
          <Trash2 size={14} />
        </Button>
      </div>
      <p className="line-clamp-3 min-h-[60px] whitespace-pre-wrap break-words text-xs leading-5 text-muted-foreground">
        {prompt.content || text("（空内容）", "(empty)", "（空）")}
      </p>
      <div className="mt-auto flex flex-wrap items-center justify-between gap-2 border-t border-border pt-3">
        <div className="min-w-0 text-[11px] text-muted-foreground">
          {prompt.enabled && (
            <p className="font-medium text-foreground">
              {matchesLive ? text("当前启用", "Active", "有効") : text("文件内容不同", "Live file differs")}
            </p>
          )}
          {validDate ? (
            <time dateTime={updated.toISOString()}>{updated.toLocaleDateString()}</time>
          ) : (
            <span>{text("更新时间不可用", "Update time unavailable")}</span>
          )}
        </div>
        <Button variant="secondary" disabled={disabled || !canWriteLive} onClick={onActivate}>
          {matchesLive ? text("重新写入", "Rewrite", "再書き込み") : text("启用", "Activate", "有効化")}
        </Button>
      </div>
    </Card>
  );
}
