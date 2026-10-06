import { useState } from "react";
import { BookOpen, FileJson } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "./ui/dialog";
import MemoryBrowser from "./openclaw-config/MemoryBrowser";

export default function OpenClawConfigSection({
  configFile,
  onOpenFile,
}: {
  configFile?: string;
  onOpenFile: (path: string) => void;
}) {
  const zh = getLocale() === "zh";
  const [open, setOpen] = useState(false);
  return (
    <div className="mb-3 flex min-w-0 flex-wrap items-center gap-2">
      <Button variant="secondary" disabled={!configFile} onClick={() => configFile && onOpenFile(configFile)}>
        <FileJson size={14} />
        {zh ? "打开原生配置" : "Open native configuration"}
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogTrigger asChild>
          <Button variant="ghost">
            <BookOpen size={14} />
            {zh ? "记忆与日志" : "Memory and journal"}
          </Button>
        </DialogTrigger>
        <DialogContent className="h-[min(760px,calc(100dvh-40px))] max-h-[calc(100dvh-40px)] max-w-[960px] max-md:h-full max-md:max-h-none">
          <DialogHeader>
            <div>
              <DialogTitle>{zh ? "记忆与日志" : "Memory and journal"}</DialogTitle>
              <DialogDescription>
                {zh
                  ? "搜索全局与项目记忆，查看完整内容；配置草稿会保留。"
                  : "Search global and project memory while retaining your configuration draft."}
              </DialogDescription>
            </div>
          </DialogHeader>
          <DialogBody className="overflow-hidden">{open && <MemoryBrowser />}</DialogBody>
        </DialogContent>
      </Dialog>
    </div>
  );
}
