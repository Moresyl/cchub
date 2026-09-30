import { useId, useState, type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
import { Button } from "./ui/button";

interface CollapsibleSectionProps {
  title: string;
  summary?: string;
  defaultOpen?: boolean;
  children: ReactNode;
}

export default function CollapsibleSection({ title, summary, defaultOpen = false, children }: CollapsibleSectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  const contentId = useId();
  const summaryId = useId();
  return (
    <div className="collapsible-section">
      <Button
        type="button"
        variant="ghost"
        className="collapsible-section-trigger"
        aria-expanded={open}
        aria-controls={contentId}
        aria-label={title}
        aria-describedby={summary ? summaryId : undefined}
        onClick={() => setOpen((current) => !current)}
      >
        <ChevronDown size={14} aria-hidden="true" className={open ? "collapsible-section-chevron-open" : undefined} />
        <span className="collapsible-section-title">{title}</span>
        {summary && (
          <span id={summaryId} className="collapsible-section-summary">
            {summary}
          </span>
        )}
      </Button>
      <div id={contentId} hidden={!open} className="collapsible-section-content">
        {open && children}
      </div>
    </div>
  );
}
