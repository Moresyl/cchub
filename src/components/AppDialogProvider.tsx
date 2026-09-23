import { AlertTriangle, Info, PencilLine } from "lucide-react";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import { Input } from "./ui/input";

type DialogTone = "default" | "warning" | "danger";

interface BaseDialogOptions {
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  tone?: DialogTone;
}

export type ConfirmOptions = BaseDialogOptions;

export interface PromptOptions extends BaseDialogOptions {
  defaultValue?: string;
  placeholder?: string;
}

interface AppDialogApi {
  confirm: (options: ConfirmOptions) => Promise<boolean>;
  prompt: (options: PromptOptions) => Promise<string | null>;
}

type DialogRequest =
  | ({ id: number; kind: "confirm"; resolve: (value: boolean) => void } & ConfirmOptions)
  | ({ id: number; kind: "prompt"; resolve: (value: string | null) => void } & PromptOptions);

const AppDialogContext = createContext<AppDialogApi | null>(null);

export function useAppDialog(): AppDialogApi {
  const value = useContext(AppDialogContext);
  if (!value) throw new Error("useAppDialog must be used inside AppDialogProvider");
  return value;
}

export function AppDialogProvider({ children }: { children: ReactNode }) {
  const [active, setActive] = useState<DialogRequest | null>(null);
  const [promptValue, setPromptValue] = useState("");
  const activeRef = useRef<DialogRequest | null>(null);
  const queueRef = useRef<DialogRequest[]>([]);
  const nextIdRef = useRef(1);

  const showNext = useCallback((request: DialogRequest | null) => {
    activeRef.current = request;
    setActive(request);
    setPromptValue(request?.kind === "prompt" ? (request.defaultValue ?? "") : "");
  }, []);

  const enqueue = useCallback(
    (request: DialogRequest) => {
      if (activeRef.current) {
        queueRef.current.push(request);
      } else {
        showNext(request);
      }
    },
    [showNext],
  );

  const confirm = useCallback(
    (options: ConfirmOptions) =>
      new Promise<boolean>((resolve) => {
        enqueue({ ...options, id: nextIdRef.current++, kind: "confirm", resolve });
      }),
    [enqueue],
  );

  const prompt = useCallback(
    (options: PromptOptions) =>
      new Promise<string | null>((resolve) => {
        enqueue({ ...options, id: nextIdRef.current++, kind: "prompt", resolve });
      }),
    [enqueue],
  );

  const finish = useCallback(
    (confirmed: boolean) => {
      const request = activeRef.current;
      if (!request) return;
      if (request.kind === "prompt") {
        request.resolve(confirmed ? promptValue : null);
      } else {
        request.resolve(confirmed);
      }
      showNext(queueRef.current.shift() ?? null);
    },
    [promptValue, showNext],
  );

  useEffect(
    () => () => {
      const pending = [activeRef.current, ...queueRef.current].filter(Boolean) as DialogRequest[];
      pending.forEach((request) => {
        if (request.kind === "confirm") request.resolve(false);
        else request.resolve(null);
      });
      activeRef.current = null;
      queueRef.current = [];
    },
    [],
  );

  const tone = active?.tone ?? "default";
  const Icon = active?.kind === "prompt" ? PencilLine : tone === "default" ? Info : AlertTriangle;
  const iconColor = tone === "danger" ? "var(--danger)" : tone === "warning" ? "var(--warning)" : "var(--accent)";
  const iconBg =
    tone === "danger" ? "var(--danger-subtle)" : tone === "warning" ? "var(--warning-subtle)" : "var(--accent-subtle)";
  const contextValue = useMemo(() => ({ confirm, prompt }), [confirm, prompt]);

  return (
    <AppDialogContext.Provider value={contextValue}>
      {children}
      <Dialog open={Boolean(active)} onOpenChange={(open) => !open && finish(false)}>
        <DialogContent hideClose className="max-w-[440px]">
          <DialogHeader>
            <div
              className="grid size-9 shrink-0 place-items-center rounded-[7px]"
              style={{ background: iconBg, color: iconColor }}
            >
              <Icon size={17} aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <DialogTitle>{active?.title ?? ""}</DialogTitle>
              <DialogDescription className="whitespace-pre-line">{active?.message ?? ""}</DialogDescription>
            </div>
          </DialogHeader>
          {active?.kind === "prompt" ? (
            <DialogBody>
              <Input
                autoFocus
                value={promptValue}
                placeholder={active.placeholder}
                onChange={(event) => setPromptValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.nativeEvent.isComposing) finish(true);
                }}
              />
            </DialogBody>
          ) : (
            <DialogBody className="hidden" />
          )}
          <DialogFooter>
            <Button variant="secondary" size="sm" onClick={() => finish(false)}>
              {active?.cancelText ?? "取消"}
            </Button>
            <Button variant={tone === "danger" ? "destructive" : "default"} size="sm" onClick={() => finish(true)}>
              {active?.confirmText ?? "确认"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </AppDialogContext.Provider>
  );
}
