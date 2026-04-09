import { memo } from "react";
import { PencilLine, Trash2 } from "lucide-react";

export interface ModelPricingListItemRow {
  model_id: string;
  normalized_model_id: string;
  input_cost_per_million: string;
  output_cost_per_million: string;
  cache_read_cost_per_million: string;
  cache_write_cost_per_million: string;
  created_at: string;
  updated_at: string;
}

interface ModelPricingListItemProps {
  item: ModelPricingListItemRow;
  deleting: boolean;
  editTitle: string;
  deleteTitle: string;
  onEdit: (item: ModelPricingListItemRow) => void;
  onDelete: (modelId: string) => void;
}

function ModelPricingListItemComponent({
  item,
  deleting,
  editTitle,
  deleteTitle,
  onEdit,
  onDelete,
}: ModelPricingListItemProps) {
  return (
    <div className="list-row" style={{ padding: "10px 12px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
      <div style={{ minWidth: 0, flex: 1 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>{item.model_id}</span>
          {item.normalized_model_id !== item.model_id && (
            <span className="badge badge-muted" style={{ fontSize: 10 }}>
              {item.normalized_model_id}
            </span>
          )}
        </div>
        <div style={{ display: "flex", gap: 12, marginTop: 4, flexWrap: "wrap", fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
          <span>in {item.input_cost_per_million}</span>
          <span>out {item.output_cost_per_million}</span>
          <span>cache-r {item.cache_read_cost_per_million}</span>
          <span>cache-w {item.cache_write_cost_per_million}</span>
        </div>
      </div>
      <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
        <button className="btn btn-secondary btn-icon-sm" onClick={() => onEdit(item)} title={editTitle}>
          <PencilLine size={14} />
        </button>
        <button
          className="btn btn-danger-ghost btn-icon-sm"
          onClick={() => onDelete(item.model_id)}
          disabled={deleting}
          title={deleteTitle}
        >
          {deleting ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Trash2 size={14} />}
        </button>
      </div>
    </div>
  );
}

export default memo(ModelPricingListItemComponent);
