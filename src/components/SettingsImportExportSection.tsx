import { useState } from "react";
import { Download, Loader2, Upload } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";

const text = (locale: string, zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);

export default function SettingsImportExportSection() {
  const locale = getLocale();
  const [busy, setBusy] = useState<"export" | "import" | null>(null);

  const exportConfig = async () => {
    setBusy("export");
    try {
      const path = await invoke<string | null>("save_file_dialog");
      if (!path) return;
      await invoke("export_config_to_file", { filePath: path });
      showToast("success", text(locale, "配置已导出", "Configuration exported", "設定をエクスポートしました"));
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setBusy(null);
    }
  };

  const importConfig = async () => {
    setBusy("import");
    try {
      const path = await invoke<string | null>("open_file_dialog");
      if (!path) return;
      await invoke("import_config_from_file", { filePath: path });
      showToast(
        "success",
        text(
          locale,
          "配置已导入，应用即将刷新",
          "Configuration imported; refreshing the app",
          "設定をインポートしました。アプリを更新します",
        ),
      );
      window.setTimeout(() => window.location.reload(), 250);
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="section-card" aria-labelledby="settings-import-export-title">
      <div className="section-card-title" id="settings-import-export-title">
        <Download size={17} style={{ color: "var(--text-secondary)" }} />
        {text(locale, "配置导入与导出", "Configuration import and export", "設定のインポートとエクスポート")}
      </div>
      <p style={{ margin: "0 0 14px", color: "var(--text-muted)", fontSize: 12, lineHeight: 1.55 }}>
        {text(
          locale,
          "导出包含配置、技能和应用设置。导入前会自动创建安全备份，导入完成后重新加载应用。",
          "Export includes profiles, skills, and app settings. Import creates a safety backup first and reloads the app when complete.",
          "設定、スキル、アプリ設定をエクスポートします。インポート前に安全バックアップを作成し、完了後にアプリを再読み込みします。",
        )}
      </p>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={() => void exportConfig()}
          disabled={busy !== null}
        >
          {busy === "export" ? <Loader2 size={14} className="spin" /> : <Download size={14} />}
          {text(locale, "导出配置", "Export", "エクスポート")}
        </button>
        <button
          type="button"
          className="btn btn-primary btn-sm"
          onClick={() => void importConfig()}
          disabled={busy !== null}
        >
          {busy === "import" ? <Loader2 size={14} className="spin" /> : <Upload size={14} />}
          {text(locale, "导入配置", "Import", "インポート")}
        </button>
      </div>
    </section>
  );
}
