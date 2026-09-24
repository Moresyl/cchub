import { type ReactNode } from "react";
import { cn } from "../../lib/cn";

interface MasterDetailLayoutProps {
  list: ReactNode;
  detail?: ReactNode;
  detailLabel?: string;
  className?: string;
  listClassName?: string;
  detailClassName?: string;
}

export default function MasterDetailLayout({
  list,
  detail,
  detailLabel,
  className,
  listClassName,
  detailClassName,
}: MasterDetailLayoutProps) {
  return (
    <div className={cn("master-detail-workspace", className)}>
      <div className={cn("master-detail-layout", detail && "master-detail-layout-open")}>
        <div className={cn("master-detail-list", listClassName)}>{list}</div>
        {detail && (
          <aside className={cn("master-detail-panel", detailClassName)} aria-label={detailLabel}>
            {detail}
          </aside>
        )}
      </div>
    </div>
  );
}
