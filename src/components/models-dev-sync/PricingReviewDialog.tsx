import { useId, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, GitCompareArrows } from "lucide-react";
import { Button } from "../ui/button";
import { Card } from "../ui/card";
import { Input } from "../ui/input";
import { SimpleSelect } from "../ui/simple-select";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";
import ErrorState from "../states/ErrorState";
import LoadingState from "../states/LoadingState";
import {
  mergePricingReview,
  pricingChanges,
  type PricingReview,
  type ReviewChoice,
  type ReviewChoices,
} from "./review";
import type { UiText } from "./types";
import "./review.css";

interface Props {
  review: PricingReview;
  text: UiText;
  onCancel: () => void;
  onRetry: () => void;
  onApply: (id: number, choices: ReviewChoices) => boolean;
  onCloseAutoFocus: (event: Event) => void;
}
export default function PricingReviewDialog({ review, text, onCancel, onRetry, onApply, onCloseAutoFocus }: Props) {
  const [choices, setChoices] = useState<ReviewChoices>({ models: new Map() });
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState("all");
  const [page, setPage] = useState(0);
  const id = useId();
  const changes = useMemo(
    () => (review.latest ? pricingChanges(review.baseline, review.draft, review.latest.config) : []),
    [review],
  );
  const unresolved = changes.filter((change) => change.conflict && !choices.models.has(change.key)).length;
  const merged = useMemo(() => mergePricingReview(review, choices), [review, choices]);
  const matches = useMemo(
    () =>
      changes.filter(
        (change) =>
          change.key.toLowerCase().includes(search.trim().toLowerCase()) && (filter !== "conflicts" || change.conflict),
      ),
    [changes, search, filter],
  );
  const pages = Math.max(1, Math.ceil(matches.length / 24));
  const currentPage = Math.min(page, pages - 1);
  const options = (conflict = false) => [
    {
      value: "",
      label: conflict
        ? text("请选择", "Choose", "選択してください")
        : text("自动合并", "Merge automatically", "自動統合"),
    },
    { value: "local", label: text("保留我的草稿", "Keep my draft", "自分の下書きを保持") },
    { value: "saved", label: text("保留已保存设置", "Keep saved settings", "保存済み設定を保持") },
  ];
  const bool = (value: boolean) => (value ? text("开启", "On", "オン") : text("关闭", "Off", "オフ"));
  const model = (value: number) =>
    [
      text("跟随常用设置", "Follow common selection", "一般モデルの設定に従う"),
      text("单独选择", "Explicitly selected", "個別に選択"),
      text("排除常用选择", "Excluded from common selection", "一般モデルから除外"),
      text(
        "单独选择 · 保留常用排除标记",
        "Explicit selection with common exclusion retained",
        "個別選択・一般モデルの除外を保持",
      ),
    ][value];
  const changeModel = (key: string, value: string) =>
    setChoices((prior) => {
      const models = new Map(prior.models);
      if (value) models.set(key, value as ReviewChoice);
      else models.delete(key);
      return { ...prior, models };
    });
  const flagRows = [
    {
      key: "autoSync" as const,
      field: "autoSyncEnabled" as const,
      label: text("启动时自动同步", "Sync on startup", "起動時に同期"),
    },
    {
      key: "common" as const,
      field: "includeCommonModels" as const,
      label: text("包含常用模型", "Include common models", "一般的なモデルを含める"),
    },
  ];
  const labels = [
    text("最初载入", "Initially loaded", "最初の設定"),
    text("已保存设置", "Saved settings", "保存済み設定"),
    text("我的草稿", "My draft", "自分の下書き"),
    text("核对后的结果", "Reviewed result", "確認後の結果"),
  ];
  return (
    <Dialog open onOpenChange={(open) => !open && onCancel()}>
      <DialogContent className="pricing-review" onCloseAutoFocus={onCloseAutoFocus}>
        <DialogHeader>
          <GitCompareArrows size={18} className="shrink-0 text-muted-foreground" aria-hidden="true" />
          <div className="min-w-0">
            <DialogTitle>{text("核对价格设置更改", "Review pricing changes", "価格設定の変更を確認")}</DialogTitle>
            <DialogDescription>
              {text(
                "保留双方各自的修改。相同模型的不同修改需要选择；应用结果后仍需保存。",
                "Keep changes from both sides. Choose between different changes to the same model; save after applying the result.",
                "双方の変更を保持します。同じモデルへの異なる変更は選択が必要です。適用後に保存してください。",
              )}
            </DialogDescription>
          </div>
        </DialogHeader>
        <DialogBody className="pricing-review-body">
          {review.loading ? (
            <LoadingState label={text("正在读取已保存设置", "Loading saved settings", "保存済み設定を読込中")} />
          ) : review.error ? (
            <ErrorState
              title={text("无法读取最新设置", "Could not load latest settings", "最新設定を読み込めませんでした")}
              message={text(
                "草稿已保留，请重试读取后再核对。",
                "Your draft is retained. Retry loading before reviewing.",
                "下書きは保持されています。再読込してから確認してください。",
              )}
              onRetry={onRetry}
              retryLabel={text("重试读取", "Retry loading", "再読込")}
            />
          ) : (
            review.latest && (
              <>
                <section
                  aria-label={text("同步选项对照", "Sync option comparison", "同期設定の比較")}
                  className="pricing-review-options"
                >
                  {flagRows.map((row) => (
                    <Card role="article" key={row.key} className="pricing-review-row shadow-none">
                      <h3>{row.label}</h3>
                      <dl className="pricing-review-values">
                        {[review.baseline[row.field], review.latest!.config[row.field], review.draft[row.field]].map(
                          (value, index) => (
                            <div key={index}>
                              <dt>{labels[index]}</dt>
                              <dd>{bool(value)}</dd>
                            </div>
                          ),
                        )}
                        <div>
                          <dt>{labels[3]}</dt>
                          <dd>
                            {bool(
                              choices[row.key] === "local"
                                ? review.draft[row.field]
                                : choices[row.key] === "saved"
                                  ? review.latest!.config[row.field]
                                  : review.draft[row.field] !== review.baseline[row.field]
                                    ? review.draft[row.field]
                                    : review.latest!.config[row.field],
                            )}
                          </dd>
                        </div>
                      </dl>
                      <SimpleSelect
                        value={choices[row.key] ?? ""}
                        options={options()}
                        ariaLabel={`${row.label} ${text("保留方式", "resolution", "保持方法")}`}
                        onValueChange={(value) => setChoices((prior) => ({ ...prior, [row.key]: value || undefined }))}
                      />
                    </Card>
                  ))}
                </section>
                <div className="pricing-review-filters">
                  <div>
                    <label htmlFor={`${id}-search`}>
                      {text("搜索模型更改", "Search model changes", "モデルの変更を検索")}
                    </label>
                    <Input
                      id={`${id}-search`}
                      value={search}
                      onChange={(event) => {
                        setSearch(event.target.value);
                        setPage(0);
                      }}
                    />
                  </div>
                  <div>
                    <label htmlFor={`${id}-filter`}>{text("显示范围", "Show", "表示範囲")}</label>
                    <SimpleSelect
                      id={`${id}-filter`}
                      value={filter}
                      options={[
                        { value: "all", label: text("全部更改", "All changes", "全ての変更") },
                        { value: "conflicts", label: text("冲突更改", "Conflicting changes", "競合する変更") },
                      ]}
                      onValueChange={(value) => {
                        setFilter(value);
                        setPage(0);
                      }}
                    />
                  </div>
                </div>
                <p className="pricing-review-status" role="status">
                  {text(
                    `共 ${changes.length} 个模型更改，${unresolved} 个冲突待选择`,
                    `${changes.length} model changes; ${unresolved} conflicts need a choice`,
                    `${changes.length} 件の変更、${unresolved} 件の競合が未解決`,
                  )}
                </p>
                <section
                  aria-label={text("模型更改对照", "Model change comparison", "モデル変更の比較")}
                  className="pricing-review-models"
                >
                  {matches.slice(currentPage * 24, (currentPage + 1) * 24).map((change) => {
                    const choice = choices.models.get(change.key);
                    const value =
                      choice === "local"
                        ? change.local
                        : choice === "saved"
                          ? change.saved
                          : change.conflict
                            ? null
                            : change.merged;
                    return (
                      <Card
                        role="article"
                        key={change.key}
                        className="pricing-review-row shadow-none"
                        data-conflict={change.conflict && !choice}
                      >
                        <h3>{change.key}</h3>
                        <dl className="pricing-review-values">
                          {[change.baseline, change.saved, change.local, value].map((value, index) => (
                            <div key={index}>
                              <dt>{labels[index]}</dt>
                              <dd>
                                {value === null ? text("需要选择", "Choose a result", "選択が必要") : model(value)}
                              </dd>
                            </div>
                          ))}
                        </dl>
                        <SimpleSelect
                          value={choice ?? ""}
                          options={options(change.conflict)}
                          ariaLabel={`${change.key} ${text("保留方式", "resolution", "保持方法")}`}
                          onValueChange={(value) => changeModel(change.key, value)}
                        />
                      </Card>
                    );
                  })}
                  {!matches.length && (
                    <p className="pricing-review-status">
                      {text("没有符合条件的模型更改", "No matching model changes", "該当するモデル変更はありません")}
                    </p>
                  )}
                </section>
                <div className="pricing-pagination">
                  <span>
                    {text(
                      `显示 ${matches.length ? currentPage * 24 + 1 : 0}–${Math.min((currentPage + 1) * 24, matches.length)} / ${matches.length}`,
                      `Showing ${matches.length ? currentPage * 24 + 1 : 0}–${Math.min((currentPage + 1) * 24, matches.length)} / ${matches.length}`,
                    )}
                  </span>
                  <div>
                    <Button
                      variant="secondary"
                      size="icon"
                      aria-label={text("上一页更改", "Previous changes", "前の変更")}
                      disabled={currentPage === 0}
                      onClick={() => setPage(currentPage - 1)}
                    >
                      <ChevronLeft size={14} />
                    </Button>
                    <span>
                      {currentPage + 1} / {pages}
                    </span>
                    <Button
                      variant="secondary"
                      size="icon"
                      aria-label={text("下一页更改", "Next changes", "次の変更")}
                      disabled={currentPage === pages - 1}
                      onClick={() => setPage(currentPage + 1)}
                    >
                      <ChevronRight size={14} />
                    </Button>
                  </div>
                </div>
                {!merged && !unresolved && (
                  <p role="alert" className="pricing-review-status">
                    {text(
                      "合并后的模型选项过多，请调整保留方式后再应用。",
                      "The merged selections exceed the limit. Adjust your choices before applying.",
                      "統合後の選択が上限を超えます。保持方法を調整してください。",
                    )}
                  </p>
                )}
              </>
            )
          )}
        </DialogBody>
        <DialogFooter className="pricing-review-footer">
          <span>
            {text(
              "应用后返回编辑，可继续调整并保存。",
              "Apply to continue editing, then save your settings.",
              "適用後、編集を続けて設定を保存できます。",
            )}
          </span>
          <div>
            <Button variant="secondary" onClick={onCancel}>
              {text("返回编辑", "Back to editing", "編集に戻る")}
            </Button>
            <Button disabled={!merged} onClick={() => onApply(review.id, choices)}>
              {text("应用核对结果", "Apply reviewed result", "確認結果を適用")}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
