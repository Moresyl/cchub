import { PackagePlus } from "lucide-react";
import type { Locale } from "../../lib/i18n";
import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";

interface PluginInstallDialogProps {
  isOpen: boolean;
  source: string;
  setSource: (value: string) => void;
  busy: boolean;
  locale: Locale;
  onConfirm: () => void;
  onCancel: () => void;
}

export default function PluginInstallDialog({
  isOpen,
  source,
  setSource,
  busy,
  locale,
  onConfirm,
  onCancel,
}: PluginInstallDialogProps) {
  const zh = locale === "zh";
  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && !busy && onCancel()}>
      <DialogContent hideClose className="max-w-[480px]">
        <DialogHeader>
          <div className="grid size-9 shrink-0 place-items-center rounded-[7px] bg-[var(--accent-subtle)] text-primary">
            <PackagePlus size={17} aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <DialogTitle>{zh ? "安装 Claude 插件" : "Install Claude plugin"}</DialogTitle>
            <DialogDescription>
              {zh
                ? "输入 HTTPS 插件归档地址，或选择本机 ZIP/TAR 文件路径。安装前会自动备份同名旧插件。"
                : "Enter an HTTPS archive URL or a local ZIP/TAR path. Existing plugins are backed up before replacement."}
            </DialogDescription>
          </div>
        </DialogHeader>
        <DialogBody>
          <Input
            value={source}
            onChange={(event) => setSource(event.target.value)}
            placeholder="https://example.com/plugin.zip"
            aria-label={zh ? "插件归档地址" : "Plugin archive URL"}
            autoFocus
            onKeyDown={(event) => {
              if (event.key === "Enter" && source.trim() && !busy) onConfirm();
            }}
          />
        </DialogBody>
        <DialogFooter>
          <Button variant="secondary" size="sm" onClick={onCancel} disabled={busy}>
            {zh ? "取消" : "Cancel"}
          </Button>
          <Button size="sm" onClick={onConfirm} disabled={!source.trim() || busy}>
            {busy ? (zh ? "安装中..." : "Installing...") : zh ? "安装" : "Install"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
