import {
  startTransition,
  useEffect,
  useEffectEvent,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  CheckCircle,
  Copy,
  Download,
  Link2,
  RefreshCw,
  Save,
  Upload,
  Wifi,
} from "lucide-react";
import { getLocale, t } from "../lib/i18n";
import { showToast } from "./Toast";

interface WebDavSyncSettings {
  enabled: boolean;
  base_url: string;
  username: string;
  password: string;
  has_password: boolean;
  remote_root: string;
  profile: string;
  auto_sync: boolean;
  last_sync_at: string | null;
  last_error: string | null;
}

interface WebDavRemoteInfo {
  exists: boolean;
  remote_url: string;
  snapshot_path: string | null;
  updated_at: string | null;
  size_bytes: number | null;
  app_version: string | null;
  device_name: string | null;
  layout: string | null;
  compatible: boolean;
  protocol_version: number | null;
  db_compat_version: number | null;
  profile_path: string | null;
}

interface WebDavSyncEvent {
  status: string;
  message: string;
  synced_at: string | null;
  error: string | null;
}

type ActionState =
  | "idle"
  | "loading"
  | "saving"
  | "testing"
  | "refreshing"
  | "uploading"
  | "downloading";

interface WebDavPreset {
  id: string;
  label: string;
  baseUrl: string;
  hint: string;
  matchPattern?: string;
}

const EMPTY_SETTINGS: WebDavSyncSettings = {
  enabled: false,
  base_url: "",
  username: "",
  password: "",
  has_password: false,
  remote_root: "cchub-sync",
  profile: "default",
  auto_sync: false,
  last_sync_at: null,
  last_error: null,
};

const WEBDAV_PRESETS: WebDavPreset[] = [
  {
    id: "jianguoyun",
    label: "坚果云",
    baseUrl: "https://dav.jianguoyun.com/dav/",
    hint: "适合直接填入坚果云账号与应用密码",
    matchPattern: "jianguoyun.com",
  },
  {
    id: "nextcloud",
    label: "Nextcloud",
    baseUrl: "https://your-server/remote.php/dav/files/USERNAME/",
    hint: "多数自建 Nextcloud / OwnCloud 使用这个路径格式",
    matchPattern: "remote.php/dav",
  },
  {
    id: "synology",
    label: "Synology",
    baseUrl: "https://your-nas:5006/",
    hint: "群晖 DSM WebDAV 常见端口为 5005/5006",
    matchPattern: ":5006",
  },
  {
    id: "custom",
    label: "自定义",
    baseUrl: "",
    hint: "任意兼容 WebDAV 的服务端地址",
  },
];

function detectPreset(baseUrl: string) {
  if (!baseUrl) return "custom";
  for (const preset of WEBDAV_PRESETS) {
    if (preset.matchPattern && baseUrl.includes(preset.matchPattern)) {
      return preset.id;
    }
  }
  return "custom";
}

function formatBytes(size: number | null) {
  if (!size || size <= 0) return "0 B";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDateTime(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

export default function WebDavSyncSection() {
  const loc = getLocale();
  const i = t();
  const uiText = (zhText: string, enText: string, jaText?: string) =>
    loc === "zh" ? zhText : loc === "ja" ? (jaText ?? enText) : enText;

  const [settings, setSettings] = useState<WebDavSyncSettings>(EMPTY_SETTINGS);
  const [remoteInfo, setRemoteInfo] = useState<WebDavRemoteInfo | null>(null);
  const [presetId, setPresetId] = useState("custom");
  const [passwordTouched, setPasswordTouched] = useState(false);
  const [actionState, setActionState] = useState<ActionState>("loading");

  const applyLoadedState = (
    nextSettings: WebDavSyncSettings,
    nextRemoteInfo: WebDavRemoteInfo | null,
  ) => {
    startTransition(() => {
      setSettings({ ...EMPTY_SETTINGS, ...nextSettings, password: "" });
      setRemoteInfo(nextRemoteInfo);
      setPresetId(detectPreset(nextSettings.base_url));
      setPasswordTouched(false);
    });
  };

  async function loadState(silent = false) {
    if (!silent) {
      setActionState("loading");
    }
    try {
      const [nextSettings, nextRemoteInfo] = await Promise.all([
        invoke<WebDavSyncSettings>("get_webdav_sync_settings"),
        invoke<WebDavRemoteInfo>("webdav_sync_fetch_remote_info").catch(
          () => null,
        ),
      ]);
      applyLoadedState(nextSettings, nextRemoteInfo);
    } catch (error) {
      if (!silent) {
        showToast("error", String(error));
      }
    } finally {
      if (!silent) {
        setActionState("idle");
      }
    }
  }

  const handleSyncEvent = useEffectEvent((payload: WebDavSyncEvent) => {
    void loadState(true);
    if (payload.status === "success") {
      showToast(
        "success",
        uiText(
          "WebDAV 自动同步已完成",
          "Automatic WebDAV sync completed",
          "WebDAV 自動同期が完了しました",
        ),
      );
      return;
    }
    if (payload.error) {
      showToast(
        "error",
        `${uiText(
          "WebDAV 自动同步失败",
          "Automatic WebDAV sync failed",
          "WebDAV 自動同期に失敗しました",
        )}: ${payload.error}`,
      );
    }
  });

  useEffect(() => {
    void loadState();
    const unlisten = listen<WebDavSyncEvent>(
      "webdav-sync-status-updated",
      (event) => handleSyncEvent(event.payload),
    );
    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, [handleSyncEvent]);

  const busy = actionState !== "idle";
  const activePreset = WEBDAV_PRESETS.find((preset) => preset.id === presetId);

  function updateSettings<K extends keyof WebDavSyncSettings>(
    key: K,
    value: WebDavSyncSettings[K],
  ) {
    startTransition(() => {
      setSettings((current) => ({ ...current, [key]: value }));
      if (key === "password") {
        setPasswordTouched(true);
      }
      if (key === "base_url") {
        setPresetId(detectPreset(String(value)));
      }
    });
  }

  async function handleCopy(value: string, label: string) {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      showToast("success", i.settings.copied.replace("{label}", label));
    } catch (error) {
      showToast("error", String(error));
    }
  }

  async function handleSave() {
    setActionState("saving");
    try {
      const saved = await invoke<WebDavSyncSettings>("set_webdav_sync_settings", {
        settings,
        passwordTouched,
      });
      applyLoadedState(saved, remoteInfo);
      showToast(
        "success",
        uiText("WebDAV 设置已保存", "WebDAV settings saved", "WebDAV 設定を保存しました"),
      );
      const nextRemoteInfo = await invoke<WebDavRemoteInfo>(
        "webdav_sync_fetch_remote_info",
      ).catch(() => null);
      startTransition(() => {
        setRemoteInfo(nextRemoteInfo);
      });
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }

  async function handleTest() {
    setActionState("testing");
    try {
      await invoke("webdav_test_connection", {
        settings,
        preserveEmptyPassword: !passwordTouched,
      });
      showToast(
        "success",
        uiText("WebDAV 连接成功", "WebDAV connection succeeded", "WebDAV 接続に成功しました"),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }

  async function refreshRemoteInfo() {
    setActionState("refreshing");
    try {
      const nextRemoteInfo = await invoke<WebDavRemoteInfo>(
        "webdav_sync_fetch_remote_info",
      );
      startTransition(() => {
        setRemoteInfo(nextRemoteInfo);
      });
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }

  async function handleUpload() {
    if (!settings.enabled) {
      showToast(
        "error",
        uiText(
          "请先启用 WebDAV 同步并保存设置",
          "Enable WebDAV sync and save settings first",
          "先に WebDAV 同期を有効化して設定を保存してください",
        ),
      );
      return;
    }
    setActionState("uploading");
    try {
      const info = await invoke<WebDavRemoteInfo>("webdav_sync_upload");
      startTransition(() => {
        setRemoteInfo(info);
      });
      await loadState(true);
      showToast(
        "success",
        uiText(
          "已上传当前 SQL 快照到 WebDAV",
          "Uploaded the current SQL snapshot to WebDAV",
          "現在の SQL スナップショットを WebDAV にアップロードしました",
        ),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }

  async function handleDownload() {
    if (!settings.enabled) {
      showToast(
        "error",
        uiText(
          "请先启用 WebDAV 同步并保存设置",
          "Enable WebDAV sync and save settings first",
          "先に WebDAV 同期を有効化して設定を保存してください",
        ),
      );
      return;
    }
    if (
      !window.confirm(
        uiText(
          "从 WebDAV 恢复会覆盖当前数据库。确认继续？",
          "Downloading from WebDAV will replace the current database. Continue?",
          "WebDAV から復元すると現在のデータベースを上書きします。続行しますか？",
        ),
      )
    ) {
      return;
    }
    setActionState("downloading");
    try {
      const message = await invoke<string>("webdav_sync_download");
      await loadState(true);
      showToast("success", message);
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }

  return (
    <div className="section-card">
      <div className="section-card-title">
        <Wifi size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText("WebDAV 云同步", "WebDAV Cloud Sync", "WebDAV クラウド同期")}
      </div>

      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
        {uiText(
          "把完整 SQL 备份上传到 WebDAV，并按 `remote_root / 协议版本 / DB 兼容版本 / profile` 组织远端目录。密码采用 backfill 策略，前端留空不会静默清除已保存密码。",
          "Upload the full SQL backup to WebDAV and organize the remote path by `remote_root / protocol version / DB compatibility version / profile`. Password handling uses backfill so leaving the field blank does not silently erase a saved password.",
          "完全な SQL バックアップを WebDAV にアップロードし、`remote_root / プロトコル版 / DB 互換版 / profile` でリモートを整理します。パスワードは backfill 方式で扱い、入力欄を空にしても保存済みパスワードは勝手に消えません。",
        )}
      </p>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
          gap: 12,
          marginBottom: 16,
        }}
      >
        <div
          style={{
            padding: "12px 14px",
            borderRadius: 10,
            background: "var(--bg-input)",
          }}
        >
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText("同步总开关", "Sync Enabled", "同期の有効化")}
          </div>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 12,
              alignItems: "center",
            }}
          >
            <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {settings.enabled
                ? uiText("已启用", "Enabled", "有効")
                : uiText("未启用", "Disabled", "無効")}
            </div>
            <button
              className={`toggle ${settings.enabled ? "on" : "off"}`}
              onClick={() => updateSettings("enabled", !settings.enabled)}
              disabled={busy}
            >
              <div className="toggle-knob" />
            </button>
          </div>
        </div>

        <div
          style={{
            padding: "12px 14px",
            borderRadius: 10,
            background: "var(--bg-input)",
          }}
        >
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText("定时自动同步", "Automatic Sync", "自動同期")}
          </div>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 12,
              alignItems: "center",
            }}
          >
            <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {settings.auto_sync
                ? uiText("每 15 分钟尝试上传一次", "Attempt upload every 15 minutes", "15 分ごとにアップロードを試行")
                : uiText("仅手动同步", "Manual sync only", "手動同期のみ")}
            </div>
            <button
              className={`toggle ${settings.auto_sync ? "on" : "off"}`}
              onClick={() => updateSettings("auto_sync", !settings.auto_sync)}
              disabled={busy}
            >
              <div className="toggle-knob" />
            </button>
          </div>
        </div>

        <div
          style={{
            padding: "12px 14px",
            borderRadius: 10,
            background: "var(--bg-input)",
          }}
        >
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText("最近同步", "Last Sync", "前回同期")}
          </div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>
            {formatDateTime(settings.last_sync_at)}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
            {remoteInfo?.layout
              ? uiText(
                  `远端布局：${remoteInfo.layout}`,
                  `Remote layout: ${remoteInfo.layout}`,
                  `リモート構成: ${remoteInfo.layout}`,
                )
              : uiText("尚未获取远端信息", "Remote info not loaded yet", "リモート情報は未取得です")}
          </div>
        </div>
      </div>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
          gap: 12,
          marginBottom: 16,
        }}
      >
        <div>
          <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
            {uiText("服务预设", "Service Preset", "サービスプリセット")}
          </div>
          <select
            className="input"
            value={presetId}
            disabled={busy}
            onChange={(event) => {
              const nextPresetId = event.target.value;
              const preset = WEBDAV_PRESETS.find((item) => item.id === nextPresetId);
              setPresetId(nextPresetId);
              if (preset && preset.id !== "custom") {
                updateSettings("base_url", preset.baseUrl);
              }
            }}
          >
            {WEBDAV_PRESETS.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.label}
              </option>
            ))}
          </select>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>
            {activePreset
              ? uiText(activePreset.hint, activePreset.hint, activePreset.hint)
              : ""}
          </div>
        </div>

        <div>
          <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
            WebDAV URL
          </div>
          <input
            className="input"
            value={settings.base_url}
            placeholder="https://dav.example.com/..."
            onChange={(event) => updateSettings("base_url", event.target.value)}
            disabled={busy}
          />
        </div>

        <div>
          <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
            {uiText("用户名", "Username", "ユーザー名")}
          </div>
          <input
            className="input"
            value={settings.username}
            onChange={(event) => updateSettings("username", event.target.value)}
            disabled={busy}
          />
        </div>

        <div>
          <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
            {uiText("密码 / 应用密码", "Password / App Password", "パスワード / アプリパスワード")}
          </div>
          <input
            className="input"
            type="password"
            value={settings.password}
            placeholder={
              settings.has_password && !passwordTouched
                ? uiText(
                    "已保存，留空则保持不变",
                    "Saved already. Leave blank to keep it.",
                    "保存済みです。空欄なら保持します。",
                  )
                : ""
            }
            onChange={(event) => updateSettings("password", event.target.value)}
            disabled={busy}
          />
        </div>

        <div>
          <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
            {uiText("远端根目录", "Remote Root", "リモートルート")}
          </div>
          <input
            className="input"
            value={settings.remote_root}
            onChange={(event) => updateSettings("remote_root", event.target.value)}
            disabled={busy}
          />
        </div>

        <div>
          <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 6 }}>
            {uiText("Profile 名称", "Profile Name", "Profile 名")}
          </div>
          <input
            className="input"
            value={settings.profile}
            onChange={(event) => updateSettings("profile", event.target.value)}
            disabled={busy}
          />
        </div>
      </div>

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
        <button
          className="btn btn-primary btn-sm"
          onClick={() => void handleSave()}
          disabled={busy}
          style={{ gap: 6 }}
        >
          <Save size={14} className={actionState === "saving" ? "spin" : ""} />
          {actionState === "saving"
            ? uiText("保存中...", "Saving...", "保存中...")
            : uiText("保存设置", "Save Settings", "設定を保存")}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          onClick={() => void handleTest()}
          disabled={busy}
          style={{ gap: 6 }}
        >
          <Wifi size={14} className={actionState === "testing" ? "spin" : ""} />
          {actionState === "testing"
            ? uiText("测试中...", "Testing...", "テスト中...")
            : uiText("测试连接", "Test Connection", "接続テスト")}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          onClick={() => void refreshRemoteInfo()}
          disabled={busy}
          style={{ gap: 6 }}
        >
          <RefreshCw size={14} className={actionState === "refreshing" ? "spin" : ""} />
          {actionState === "refreshing"
            ? uiText("刷新中...", "Refreshing...", "更新中...")
            : uiText("刷新远端", "Refresh Remote", "リモートを更新")}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          onClick={() => void handleUpload()}
          disabled={busy}
          style={{ gap: 6 }}
        >
          <Upload size={14} className={actionState === "uploading" ? "spin" : ""} />
          {actionState === "uploading"
            ? uiText("上传中...", "Uploading...", "アップロード中...")
            : uiText("上传当前快照", "Upload Snapshot", "現在のスナップショットをアップロード")}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          onClick={() => void handleDownload()}
          disabled={busy || !remoteInfo?.exists}
          style={{ gap: 6 }}
        >
          <Download size={14} className={actionState === "downloading" ? "spin" : ""} />
          {actionState === "downloading"
            ? uiText("恢复中...", "Restoring...", "復元中...")
            : uiText("从远端恢复", "Restore From Remote", "リモートから復元")}
        </button>
        {remoteInfo?.remote_url && (
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => void handleCopy(remoteInfo.remote_url, "WebDAV URL")}
            disabled={busy}
            style={{ gap: 6 }}
          >
            <Copy size={14} />
            {uiText("复制远端地址", "Copy Remote URL", "リモート URL をコピー")}
          </button>
        )}
      </div>

      {settings.has_password && !passwordTouched && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            fontSize: 12,
            color: "var(--text-secondary)",
            marginBottom: 12,
          }}
        >
          <CheckCircle size={14} style={{ color: "var(--success)" }} />
          {uiText(
            "后端已保存密码，当前输入框为空不会清除它；只有你实际修改密码输入框后才会覆盖。",
            "A password is already stored on the backend. Leaving the field empty will keep it; only touching the password field will replace it.",
            "パスワードは既にバックエンドへ保存されています。入力欄を空のままにしても保持され、パスワード欄を実際に編集した場合のみ上書きされます。",
          )}
        </div>
      )}

      {settings.last_error && (
        <div
          style={{
            padding: "12px 14px",
            borderRadius: 10,
            background: "color-mix(in srgb, var(--danger) 8%, var(--bg-input))",
            color: "var(--text-primary)",
            marginBottom: 16,
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "flex-start",
              gap: 8,
              fontSize: 12,
            }}
          >
            <AlertCircle size={14} style={{ color: "var(--danger)", marginTop: 1 }} />
            <div>
              <div style={{ fontWeight: 600, marginBottom: 4 }}>
                {uiText("最近一次同步错误", "Last Sync Error", "前回同期エラー")}
              </div>
              <div>{settings.last_error}</div>
            </div>
          </div>
        </div>
      )}

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
          gap: 12,
        }}
      >
        <div
          style={{
            padding: "12px 14px",
            borderRadius: 10,
            background: "var(--bg-input)",
          }}
        >
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText("远端状态", "Remote Status", "リモート状態")}
          </div>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              fontSize: 13,
              fontWeight: 600,
            }}
          >
            {remoteInfo?.exists ? (
              remoteInfo.compatible ? (
                <CheckCircle size={15} style={{ color: "var(--success)" }} />
              ) : (
                <AlertCircle size={15} style={{ color: "var(--warning)" }} />
              )
            ) : (
              <Link2 size={15} style={{ color: "var(--text-secondary)" }} />
            )}
            {remoteInfo?.exists
              ? remoteInfo.compatible
                ? uiText("已发现可兼容快照", "Compatible snapshot found", "互換スナップショットを検出")
                : uiText("发现远端快照，但版本不兼容", "Remote snapshot found but incompatible", "リモートスナップショットを検出しましたが互換性がありません")
              : uiText("远端暂无快照", "No remote snapshot yet", "リモートにスナップショットはありません")}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
            {remoteInfo?.updated_at
              ? formatDateTime(remoteInfo.updated_at)
              : uiText("等待首次上传", "Waiting for the first upload", "最初のアップロード待ち")}
          </div>
        </div>

        <div
          style={{
            padding: "12px 14px",
            borderRadius: 10,
            background: "var(--bg-input)",
          }}
        >
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText("远端快照体积", "Remote Snapshot Size", "リモートスナップショットサイズ")}
          </div>
          <div style={{ fontSize: 20, fontWeight: 700 }}>
            {formatBytes(remoteInfo?.size_bytes ?? null)}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
            {remoteInfo?.app_version
              ? `CCHub ${remoteInfo.app_version}`
              : uiText("尚未获取版本信息", "Version info unavailable", "バージョン情報は未取得です")}
          </div>
        </div>

        <div
          style={{
            padding: "12px 14px",
            borderRadius: 10,
            background: "var(--bg-input)",
          }}
        >
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText("远端路径", "Remote Path", "リモートパス")}
          </div>
          <div
            style={{
              fontSize: 12,
              fontFamily: "'JetBrains Mono', monospace",
              wordBreak: "break-all",
            }}
          >
            {remoteInfo?.profile_path || "—"}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
            {remoteInfo?.protocol_version || remoteInfo?.db_compat_version
              ? `v${remoteInfo.protocol_version ?? "?"} / db-v${remoteInfo.db_compat_version ?? "?"}`
              : uiText("未读取到版本层级", "Versioned path not loaded", "バージョン階層は未取得です")}
          </div>
        </div>
      </div>

      {remoteInfo?.snapshot_path && (
        <div
          style={{
            marginTop: 16,
            padding: "12px 14px",
            borderRadius: 10,
            background: "var(--bg-input)",
          }}
        >
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: 12,
              alignItems: "center",
              flexWrap: "wrap",
              marginBottom: 8,
            }}
          >
            <div style={{ fontSize: 13, fontWeight: 600 }}>
              {uiText("远端快照详情", "Remote Snapshot Details", "リモートスナップショット詳細")}
            </div>
            {remoteInfo.remote_url && (
              <button
                className="btn btn-ghost btn-icon-sm"
                onClick={() => void handleCopy(remoteInfo.remote_url, "WebDAV URL")}
                title={uiText("复制 manifest 地址", "Copy manifest URL", "manifest URL をコピー")}
              >
                <Copy size={12} />
              </button>
            )}
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 6, fontSize: 12 }}>
            <div>
              <span style={{ color: "var(--text-muted)" }}>
                {uiText("快照文件", "Snapshot", "スナップショット")}
                {" : "}
              </span>
              <code>{remoteInfo.snapshot_path}</code>
            </div>
            <div>
              <span style={{ color: "var(--text-muted)" }}>
                {uiText("设备名", "Device", "デバイス")}
                {" : "}
              </span>
              {remoteInfo.device_name || "—"}
            </div>
            <div>
              <span style={{ color: "var(--text-muted)" }}>
                {uiText("Manifest 地址", "Manifest URL", "Manifest URL")}
                {" : "}
              </span>
              <code style={{ wordBreak: "break-all" }}>{remoteInfo.remote_url}</code>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
