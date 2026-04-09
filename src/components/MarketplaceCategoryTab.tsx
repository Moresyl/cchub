import { memo } from "react";

interface MarketplaceCategoryTabProps {
  categoryKey: string;
  label: string;
  active: boolean;
  onSelect: (categoryKey: string) => void;
}

function MarketplaceCategoryTabComponent({
  categoryKey,
  label,
  active,
  onSelect,
}: MarketplaceCategoryTabProps) {
  return (
    <button
      className={`tab-item ${active ? "active" : ""}`}
      onClick={() => onSelect(categoryKey)}
    >
      {label}
    </button>
  );
}

export default memo(MarketplaceCategoryTabComponent);
