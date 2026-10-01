import { useMemo, useState } from "react";
import type { CopilotModel } from "../../lib/copilotAccounts";
import { normalizeModelBilling } from "../../lib/modelBilling";
import { Input } from "../ui/input";
import { SimpleSelect } from "../ui/simple-select";
import ModelBillingBadge, { type BillingText } from "../ModelBillingBadge";

export default function ModelsList({ models, id, text }: { models: CopilotModel[]; id: string; text: BillingText }) {
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState("all");
  const visible = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    return models.filter(
      (model) =>
        (filter === "all" || normalizeModelBilling(model.billing).kind === filter) &&
        `${model.id} ${model.name} ${model.vendor}`.toLocaleLowerCase().includes(query),
    );
  }, [models, search, filter]);
  return (
    <div id={id} className="grid min-w-0 gap-3">
      <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto]">
        <Input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          aria-label={text("搜索 Copilot 模型", "Search Copilot models", "Copilot モデルを検索")}
          placeholder={text("搜索名称、ID 或供应商", "Search name, ID or vendor", "名前、ID、提供元を検索")}
        />
        <SimpleSelect
          value={filter}
          onValueChange={setFilter}
          ariaLabel={text("模型计费筛选", "Model billing filter", "モデル課金フィルター")}
          options={[
            { value: "all", label: text("全部计费类型", "All billing types", "すべての課金種別") },
            { value: "free", label: text("不消耗高级额度", "No premium quota", "Premium クォータ消費なし") },
            { value: "premium", label: text("高级请求", "Premium requests", "Premium リクエスト") },
            { value: "unknown", label: text("计费信息未提供", "Billing not reported", "課金情報未報告") },
          ]}
        />
      </div>
      <p className="text-[11px] text-muted-foreground" role="status">
        {text(
          `显示 ${visible.length} / ${models.length} 个模型`,
          `Showing ${visible.length} / ${models.length} models`,
          `${models.length} モデル中 ${visible.length} 件表示`,
        )}
      </p>
      <p className="text-[12px] text-muted-foreground">
        {text(
          "倍率表示单次请求消耗的高级额度，不是 token 单价。基础聊天额度与订阅限制仍适用。",
          "Multipliers describe premium-request units, not token prices. Chat quota and subscription limits still apply.",
          "倍率は Premium リクエストの消費量であり、トークン単価ではありません。チャットのクォータと契約制限は引き続き適用されます。",
        )}
      </p>
      {visible.length > 0 ? (
        <ul
          className="grid max-h-72 gap-2 overflow-y-auto pr-1"
          aria-label={text("Copilot 模型列表", "Copilot models", "Copilot モデル一覧")}
        >
          {visible.map((model) => (
            <li key={model.id} className="grid min-w-0 gap-2 rounded-md bg-[var(--bg-input)] p-3">
              <div className="flex flex-wrap items-start justify-between gap-2">
                <span className="min-w-0 break-all">{model.name || model.id}</span>
                <ModelBillingBadge value={model.billing} localeText={text} />
              </div>
              <span className="break-all font-mono text-[11px] text-muted-foreground">
                {model.id}
                {model.vendor ? ` · ${model.vendor}` : ""}
              </span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-[12px] text-muted-foreground">
          {text("没有符合条件的模型", "No matching models", "一致するモデルがありません")}
        </p>
      )}
    </div>
  );
}
