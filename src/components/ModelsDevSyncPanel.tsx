import { useCallback, useEffect, useId, useRef, useState, type MouseEvent } from "react";
import { ChevronDown, ChevronUp, CloudDownload, RefreshCw, Save } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { Button } from "./ui/button";
import { CheckboxField } from "./ui/checkbox-field";
import ConfirmDialog from "./ConfirmDialog";
import ErrorState from "./states/ErrorState";
import LoadingState from "./states/LoadingState";
import ModelPicker from "./models-dev-sync/ModelPicker";
import PricingReviewDialog from "./models-dev-sync/PricingReviewDialog";
import { usePricingSettings } from "./models-dev-sync/usePricingSettings";
import { selectModel } from "./models-dev-sync/types";
import "./models-dev-sync/styles.css";

export default function ModelsDevSyncPanel() {
  const locale = getLocale();
  const text = useCallback(
    (zh: string, en: string, ja?: string) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en),
    [locale],
  );
  const settings = usePricingSettings();
  const [open, setOpen] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);
  const resetTrigger = useRef<HTMLButtonElement | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const reviewTrigger = useRef<HTMLButtonElement | null>(null);
  const reviewOpen = useRef(false);
  reviewOpen.current = !!settings.review;
  const openReset = (event: MouseEvent<HTMLButtonElement>) => {
    resetTrigger.current = event.currentTarget;
    setConfirmReset(true);
  };
  const id = useId();
  useEffect(() => {
    const save = () => {
      if (settings.dirty && !settings.blocked) void settings.save();
    };
    window.addEventListener("cchub-shortcut-save", save);
    return () => window.removeEventListener("cchub-shortcut-save", save);
  }, [settings]);
  const config = settings.draft;
  return (
    <section className="section-card pricing-sync" aria-labelledby={`${id}-title`}>
      <div className="pricing-heading">
        <div>
          <h2 ref={heading} tabIndex={-1} id={`${id}-title`} className="section-card-title">
            <CloudDownload size={16} aria-hidden="true" />
            {text("模型价格同步", "Model pricing sync", "モデル価格の同期")}
          </h2>
          <p className="pricing-help">
            {text(
              "从公开模型目录更新本地价格，用于代理成本统计。",
              "Update local model prices for proxy cost reports.",
              "公開モデルカタログの価格をプロキシのコスト集計に使用します。",
            )}
          </p>
        </div>
        <Button
          type="button"
          variant="secondary"
          size="icon"
          disabled={!!settings.busy || settings.loading}
          onClick={() => void settings.refresh()}
          aria-label={text("刷新同步状态", "Refresh sync status", "同期状態を更新")}
          title={text("刷新同步状态", "Refresh sync status", "同期状態を更新")}
        >
          <RefreshCw size={15} />
        </Button>
      </div>
      {settings.loading && !config && (
        <div role="status">
          <LoadingState label={text("正在读取同步设置", "Loading sync settings", "同期設定を読込中")} />
        </div>
      )}
      {settings.failure === "read" && (
        <ErrorState
          title={text("同步设置读取失败", "Could not load sync settings", "同期設定を読み込めませんでした")}
          message={text(
            "已有更改已保留。重新读取成功后才能保存或同步。",
            "Changes are retained. Reload settings before saving or syncing.",
            "変更は保持されています。設定を再読込してから保存・同期してください。",
          )}
          retryLabel={text("重试", "Retry", "再試行")}
          onRetry={settings.loading ? undefined : () => void settings.refresh()}
        />
      )}
      {config && (
        <>
          <div className="pricing-options">
            <CheckboxField
              variant="surface"
              checked={config.autoSyncEnabled}
              disabled={!!settings.busy || settings.loading}
              onCheckedChange={(checked) => settings.update((draft) => ({ ...draft, autoSyncEnabled: checked }))}
              label={text("启动时自动同步", "Sync on startup", "起動時に同期")}
              description={text("最多每 6 小时执行一次", "At most once every 6 hours", "最短 6 時間間隔")}
            />
            <CheckboxField
              variant="surface"
              checked={config.includeCommonModels}
              disabled={!!settings.busy || settings.loading}
              onCheckedChange={(checked) => settings.update((draft) => ({ ...draft, includeCommonModels: checked }))}
              label={text("包含常用模型", "Include common models", "一般的なモデルを含める")}
              description={text(
                "每个常用供应商最多保留 6 个近期模型，可单独取消勾选。",
                "Up to 6 recent models per common provider; deselect any individually.",
                "一般的なProviderごとに最近の6モデルまで。個別に解除できます。",
              )}
            />
          </div>
          <div className="pricing-status">
            <span>
              {text("上次同步", "Last sync", "最終同期")}:{" "}
              {settings.state?.config.lastSyncAt
                ? new Date(settings.state.config.lastSyncAt).toLocaleString()
                : text("从未同步", "Never synced", "未同期")}
            </span>
            {settings.state?.configPath && (
              <details>
                <summary>{text("本地价格文件", "Local pricing file", "ローカル価格ファイル")}</summary>
                <code>{settings.state.configPath}</code>
              </details>
            )}
          </div>
          {(settings.failure === "save" || settings.failure === "sync" || settings.failure === "conflict") && (
            <div className="pricing-notice" role="alert">
              {settings.failure === "conflict"
                ? text(
                    "设置已在其他操作中更改。草稿已保留，可以核对双方修改后继续保存。",
                    "Settings changed elsewhere. Your draft is retained; review changes from both sides before saving.",
                    "別の操作で設定が変更されました。下書きを保持しています。確認するか、再読込して下書きを破棄してください。",
                  )
                : settings.failure === "save"
                  ? text(
                      "设置保存失败，草稿已保留。重试保存后再同步。",
                      "Settings could not be saved. Your draft is retained; retry saving before syncing.",
                      "保存できませんでした。下書きは保持されています。保存を再試行してください。",
                    )
                  : text(
                      "价格同步失败，已保存的选择不受影响。检查网络后重试同步。",
                      "Pricing sync failed. Saved selections are retained. Check the connection and retry.",
                      "価格同期に失敗しました。保存済みの選択は保持されています。接続を確認して再試行してください。",
                    )}
              {settings.failure === "conflict" && (
                <>
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={!!settings.busy || settings.loading}
                    onClick={(event) => {
                      reviewTrigger.current = event.currentTarget;
                      void settings.reviewChanges();
                    }}
                  >
                    {text("核对更改", "Review changes", "変更を確認")}
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={!!settings.busy || settings.loading}
                    onClick={openReset}
                  >
                    {text("重新加载", "Reload", "再読込")}
                  </Button>
                </>
              )}
            </div>
          )}
          {!settings.failure && settings.state?.config.lastSyncError && (
            <p className="pricing-notice">
              {text(
                "上次自动同步未完成，可手动重试。",
                "The last automatic sync failed. Try a manual sync.",
                "前回の自動同期は未完了です。手動で再試行できます。",
              )}
            </p>
          )}
          <div className="pricing-picker-toggle">
            <span>
              {text(
                `${config.selectedModelKeys.length} 个显式模型`,
                `${config.selectedModelKeys.length} explicit models`,
                `${config.selectedModelKeys.length} 件の明示モデル`,
              )}
            </span>
            <Button
              type="button"
              variant="secondary"
              onClick={() => setOpen(!open)}
              aria-expanded={open}
              aria-controls={`${id}-picker`}
            >
              {open ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
              {open
                ? text("收起选择器", "Hide picker", "選択を閉じる")
                : text("选择模型", "Choose models", "モデルを選択")}
            </Button>
          </div>
          <div id={`${id}-picker`} hidden={!open}>
            {open && (
              <ModelPicker
                config={config}
                disabled={!!settings.busy || settings.loading}
                text={text}
                onSelect={(entry, checked) => settings.update((draft) => selectModel(entry, draft, checked))}
              />
            )}
          </div>
          <div className="pricing-actions">
            <span role="status">
              {settings.busy
                ? text(
                    settings.busy === "save" ? "正在保存设置…" : "正在同步价格…",
                    settings.busy === "save" ? "Saving settings…" : "Syncing prices…",
                    settings.busy === "save" ? "保存中…" : "価格を同期中…",
                  )
                : settings.dirty
                  ? text(
                      "更改尚未保存；切换页面后草稿会保留。",
                      "Unsaved changes; your draft is retained across pages.",
                      "未保存の変更はページ切替後も保持されます。",
                    )
                  : settings.result
                    ? text(
                        `已同步 ${settings.result.imported} 个模型，更新 ${settings.result.changed} 项`,
                        `Synced ${settings.result.imported} models, changed ${settings.result.changed}`,
                        `${settings.result.imported} 件を同期、${settings.result.changed} 件を更新`,
                      )
                    : text("设置已保存", "Settings saved", "設定は保存済み")}
            </span>
            <div>
              <Button type="button" variant="ghost" disabled={settings.blocked || !settings.dirty} onClick={openReset}>
                {text("放弃更改", "Discard changes", "変更を破棄")}
              </Button>
              <Button
                type="button"
                variant="secondary"
                disabled={settings.blocked || !settings.dirty}
                onClick={() => void settings.save()}
              >
                <Save size={14} />
                {text("保存设置", "Save settings", "設定を保存")}
              </Button>
              <Button type="button" disabled={settings.blocked} onClick={() => void settings.sync()}>
                <CloudDownload size={14} />
                {settings.dirty
                  ? text("保存并同步", "Save and sync", "保存して同期")
                  : text("立即同步", "Sync now", "今すぐ同期")}
              </Button>
            </div>
          </div>
        </>
      )}
      <ConfirmDialog
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          const trigger = resetTrigger.current;
          if (trigger?.isConnected && !trigger.disabled) trigger.focus();
          else heading.current?.focus();
        }}
        isOpen={confirmReset}
        variant="info"
        title={text("放弃未保存的更改？", "Discard unsaved changes?", "未保存の変更を破棄しますか？")}
        message={text(
          "重新读取已保存设置。读取失败时仍保留你的草稿。",
          "Reload saved settings. If reloading fails, your draft is retained.",
          "保存済み設定を再読込します。失敗した場合は下書きを保持します。",
        )}
        confirmText={text("放弃并重新加载", "Discard and reload", "破棄して再読込")}
        cancelText={text("继续编辑", "Keep editing", "編集を続ける")}
        onConfirm={() => {
          setConfirmReset(false);
          void settings.refresh(true);
        }}
        onCancel={() => setConfirmReset(false)}
      />
      {settings.review && (
        <PricingReviewDialog
          key={settings.review.id}
          review={settings.review}
          text={text}
          onCancel={settings.cancelReview}
          onRetry={() => void settings.reviewChanges()}
          onApply={settings.applyReview}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            if (reviewOpen.current) return;
            const trigger = reviewTrigger.current;
            if (trigger?.isConnected && !trigger.disabled) trigger.focus();
            else heading.current?.focus();
          }}
        />
      )}
    </section>
  );
}
