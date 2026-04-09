import {
  memo,
  startTransition,
  useCallback,
  useEffect,
  useEffectEvent,
  useMemo,
  useState,
  type ChangeEvent,
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
  type LucideIcon,
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

type WebDavTextFieldKey =
  | "base_url"
  | "username"
  | "password"
  | "remote_root"
  | "profile";

interface WebDavToggleCardProps {
  title: string;
  description: string;
  enabled: boolean;
  disabled: boolean;
  onToggle: () => void;
}

interface WebDavTextFieldProps {
  fieldKey: WebDavTextFieldKey;
  label: string;
  value: string;
  disabled: boolean;
  onValueChange: (fieldKey: WebDavTextFieldKey, value: string) => void;
  placeholder?: string;
  type?: "text" | "password";
}

interface WebDavActionButtonProps {
  label: string;
  loading: boolean;
  disabled: boolean;
  icon: LucideIcon;
  variant?: "btn-primary" | "btn-secondary" | "btn-ghost";
  onClick: () => void;
}

interface WebDavInfoCardProps {
  title: string;
  value: string;
  detail: string;
  valueLarge?: boolean;
  mono?: boolean;
  icon?: LucideIcon;
  iconColor?: string;
}

interface WebDavSnapshotDetailsProps {
  title: string;
  snapshotLabel: string;
  snapshotPath: string;
  deviceLabel: string;
  deviceName: string;
  manifestLabel: string;
  remoteUrl: string;
  copyTitle: string;
  canCopy: boolean;
  onCopy: () => void;
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

function WebDavToggleCardComponent({
  title,
  description,
  enabled,
  disabled,
  onToggle,
}: WebDavToggleCardProps) {
  return (
    <div
      style={{
        padding: "12px 14px",
        borderRadius: 10,
        background: "var(--bg-input)",
      }}
    >
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
        {title}
      </div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 12,
          alignItems: "center",
        }}
      >
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>{description}</div>
        <button
          className={`toggle ${enabled ? "on" : "off"}`}
          onClick={onToggle}
          disabled={disabled}
        >
          <div className="toggle-knob" />
        </button>
      </div>
    </div>
  );
}

const WebDavToggleCard = memo(WebDavToggleCardComponent);

function WebDavTextFieldComponent({
  fieldKey,
  label,
  value,
  disabled,
  onValueChange,
  placeholder,
  type = "text",
}: WebDavTextFieldProps) {
  const handleChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    onValueChange(fieldKey, event.target.value);
  }, [fieldKey, onValueChange]);

  return (
    <div>
      <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 6 }}>{label}</div>
      <input
        className="input"
        value={value}
        placeholder={placeholder}
        onChange={handleChange}
        disabled={disabled}
        type={type}
      />
    </div>
  );
}

const WebDavTextField = memo(WebDavTextFieldComponent);

function WebDavActionButtonComponent({
  label,
  loading,
  disabled,
  icon: Icon,
  variant = "btn-secondary",
  onClick,
}: WebDavActionButtonProps) {
  return (
    <button
      className={`btn ${variant} btn-sm`}
      onClick={onClick}
      disabled={disabled}
      style={{ gap: 6 }}
    >
      <Icon size={14} className={loading ? "spin" : ""} />
      {label}
    </button>
  );
}

const WebDavActionButton = memo(WebDavActionButtonComponent);

function WebDavInfoCardComponent({
  title,
  value,
  detail,
  valueLarge = false,
  mono = false,
  icon: Icon,
  iconColor,
}: WebDavInfoCardProps) {
  return (
    <div
      style={{
        padding: "12px 14px",
        borderRadius: 10,
        background: "var(--bg-input)",
      }}
    >
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
        {title}
      </div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          fontSize: valueLarge ? 20 : 13,
          fontWeight: valueLarge ? 700 : 600,
          fontFamily: mono ? "'JetBrains Mono', monospace" : undefined,
          wordBreak: mono ? "break-all" : undefined,
        }}
      >
        {Icon ? <Icon size={15} style={{ color: iconColor }} /> : null}
        <span>{value}</span>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
        {detail}
      </div>
    </div>
  );
}

const WebDavInfoCard = memo(WebDavInfoCardComponent);

function WebDavSnapshotDetailsComponent({
  title,
  snapshotLabel,
  snapshotPath,
  deviceLabel,
  deviceName,
  manifestLabel,
  remoteUrl,
  copyTitle,
  canCopy,
  onCopy,
}: WebDavSnapshotDetailsProps) {
  return (
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
        <div style={{ fontSize: 13, fontWeight: 600 }}>{title}</div>
        {canCopy ? (
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={onCopy}
            title={copyTitle}
          >
            <Copy size={12} />
          </button>
        ) : null}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6, fontSize: 12 }}>
        <div>
          <span style={{ color: "var(--text-muted)" }}>
            {snapshotLabel}
            {" : "}
          </span>
          <code>{snapshotPath}</code>
        </div>
        <div>
          <span style={{ color: "var(--text-muted)" }}>
            {deviceLabel}
            {" : "}
          </span>
          {deviceName}
        </div>
        <div>
          <span style={{ color: "var(--text-muted)" }}>
            {manifestLabel}
            {" : "}
          </span>
          <code style={{ wordBreak: "break-all" }}>{remoteUrl}</code>
        </div>
      </div>
    </div>
  );
}

const WebDavSnapshotDetails = memo(WebDavSnapshotDetailsComponent);

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

function WebDavSyncSectionComponent() {
  const loc = getLocale();
  const i = t();
  const uiText = useCallback((zhText: string, enText: string, jaText?: string) => (
    loc === "zh" ? zhText : loc === "ja" ? (jaText ?? enText) : enText
  ), [loc]);

  const [settings, setSettings] = useState<WebDavSyncSettings>(EMPTY_SETTINGS);
  const [remoteInfo, setRemoteInfo] = useState<WebDavRemoteInfo | null>(null);
  const [presetId, setPresetId] = useState("custom");
  const [passwordTouched, setPasswordTouched] = useState(false);
  const [actionState, setActionState] = useState<ActionState>("loading");

  const applyLoadedState = useCallback((
    nextSettings: WebDavSyncSettings,
    nextRemoteInfo: WebDavRemoteInfo | null,
  ) => {
    startTransition(() => {
      setSettings({ ...EMPTY_SETTINGS, ...nextSettings, password: "" });
      setRemoteInfo(nextRemoteInfo);
      setPresetId(detectPreset(nextSettings.base_url));
      setPasswordTouched(false);
    });
  }, []);

  const loadState = useCallback(async (silent = false) => {
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
  }, [applyLoadedState]);

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
  const activePreset = useMemo(
    () => WEBDAV_PRESETS.find((preset) => preset.id === presetId),
    [presetId],
  );

  const updateSettings = useCallback(<K extends keyof WebDavSyncSettings,>(
    key: K,
    value: WebDavSyncSettings[K],
  ) => {
    startTransition(() => {
      setSettings((current) => ({ ...current, [key]: value }));
      if (key === "password") {
        setPasswordTouched(true);
      }
      if (key === "base_url") {
        setPresetId(detectPreset(String(value)));
      }
    });
  }, []);

  const handleCopy = useCallback(async (value: string, label: string) => {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      showToast("success", i.settings.copied.replace("{label}", label));
    } catch (error) {
      showToast("error", String(error));
    }
  }, [i.settings.copied]);

  const handleSave = useCallback(async () => {
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
  }, [applyLoadedState, passwordTouched, remoteInfo, settings, uiText]);

  const handleTest = useCallback(async () => {
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
  }, [passwordTouched, settings, uiText]);

  const refreshRemoteInfo = useCallback(async () => {
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
  }, []);

  const handleUpload = useCallback(async () => {
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
  }, [loadState, settings.enabled, uiText]);

  const handleDownload = useCallback(async () => {
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
  }, [loadState, settings.enabled, uiText]);

  const handleToggleEnabled = useCallback(() => {
    updateSettings("enabled", !settings.enabled);
  }, [settings.enabled, updateSettings]);

  const handleToggleAutoSync = useCallback(() => {
    updateSettings("auto_sync", !settings.auto_sync);
  }, [settings.auto_sync, updateSettings]);

  const handleValueChange = useCallback((fieldKey: WebDavTextFieldKey, value: string) => {
    updateSettings(fieldKey, value);
  }, [updateSettings]);

  const handlePresetChange = useCallback((event: ChangeEvent<HTMLSelectElement>) => {
    const nextPresetId = event.target.value;
    const preset = WEBDAV_PRESETS.find((item) => item.id === nextPresetId);
    setPresetId(nextPresetId);
    if (preset && preset.id !== "custom") {
      updateSettings("base_url", preset.baseUrl);
    }
  }, [updateSettings]);

  const handleSaveClick = useCallback(() => {
    void handleSave();
  }, [handleSave]);

  const handleTestClick = useCallback(() => {
    void handleTest();
  }, [handleTest]);

  const handleRefreshRemoteClick = useCallback(() => {
    void refreshRemoteInfo();
  }, [refreshRemoteInfo]);

  const handleUploadClick = useCallback(() => {
    void handleUpload();
  }, [handleUpload]);

  const handleDownloadClick = useCallback(() => {
    void handleDownload();
  }, [handleDownload]);

  const handleCopyRemoteUrl = useCallback(() => {
    if (!remoteInfo?.remote_url) return;
    void handleCopy(remoteInfo.remote_url, "WebDAV URL");
  }, [handleCopy, remoteInfo?.remote_url]);

  const handleCopyManifestUrl = useCallback(() => {
    if (!remoteInfo?.remote_url) return;
    void handleCopy(remoteInfo.remote_url, "WebDAV URL");
  }, [handleCopy, remoteInfo?.remote_url]);

  const remoteLayoutLabel = useMemo(() => (
    remoteInfo?.layout
      ? uiText(
          `远端布局：${remoteInfo.layout}`,
          `Remote layout: ${remoteInfo.layout}`,
          `リモート構成: ${remoteInfo.layout}`,
        )
      : uiText("尚未获取远端信息", "Remote info not loaded yet", "リモート情報は未取得です")
  ), [remoteInfo?.layout, uiText]);

  const passwordPlaceholder = useMemo(() => (
    settings.has_password && !passwordTouched
      ? uiText(
          "已保存，留空则保持不变",
          "Saved already. Leave blank to keep it.",
          "保存済みです。空欄なら保持します。",
        )
      : ""
  ), [passwordTouched, settings.has_password, uiText]);

  const presetHint = useMemo(() => (
    activePreset ? uiText(activePreset.hint, activePreset.hint, activePreset.hint) : ""
  ), [activePreset, uiText]);

  const settingsFields = useMemo<WebDavTextFieldProps[]>(() => [
    {
      fieldKey: "base_url",
      label: "WebDAV URL",
      value: settings.base_url,
      placeholder: "https://dav.example.com/...",
      disabled: busy,
      onValueChange: handleValueChange,
    },
    {
      fieldKey: "username",
      label: uiText("用户名", "Username", "ユーザー名"),
      value: settings.username,
      disabled: busy,
      onValueChange: handleValueChange,
    },
    {
      fieldKey: "password",
      label: uiText("密码 / 应用密码", "Password / App Password", "パスワード / アプリパスワード"),
      value: settings.password,
      placeholder: passwordPlaceholder,
      type: "password",
      disabled: busy,
      onValueChange: handleValueChange,
    },
    {
      fieldKey: "remote_root",
      label: uiText("远端根目录", "Remote Root", "リモートルート"),
      value: settings.remote_root,
      disabled: busy,
      onValueChange: handleValueChange,
    },
    {
      fieldKey: "profile",
      label: uiText("Profile 名称", "Profile Name", "Profile 名"),
      value: settings.profile,
      disabled: busy,
      onValueChange: handleValueChange,
    },
  ], [
    busy,
    handleValueChange,
    passwordPlaceholder,
    settings.base_url,
    settings.password,
    settings.profile,
    settings.remote_root,
    settings.username,
    uiText,
  ]);

  const remoteStatusIcon = remoteInfo?.exists
    ? (remoteInfo.compatible ? CheckCircle : AlertCircle)
    : Link2;
  const remoteStatusColor = remoteInfo?.exists
    ? (remoteInfo.compatible ? "var(--success)" : "var(--warning)")
    : "var(--text-secondary)";
  const remoteStatusValue = remoteInfo?.exists
    ? remoteInfo.compatible
      ? uiText("已发现可兼容快照", "Compatible snapshot found", "互換スナップショットを検出")
      : uiText("发现远端快照，但版本不兼容", "Remote snapshot found but incompatible", "リモートスナップショットを検出しましたが互換性がありません")
    : uiText("远端暂无快照", "No remote snapshot yet", "リモートにスナップショットはありません");
  const remoteStatusDetail = remoteInfo?.updated_at
    ? formatDateTime(remoteInfo.updated_at)
    : uiText("等待首次上传", "Waiting for the first upload", "最初のアップロード待ち");
  const remoteSizeDetail = remoteInfo?.app_version
    ? `CCHub ${remoteInfo.app_version}`
    : uiText("尚未获取版本信息", "Version info unavailable", "バージョン情報は未取得です");
  const remotePathDetail = remoteInfo?.protocol_version || remoteInfo?.db_compat_version
    ? `v${remoteInfo.protocol_version ?? "?"} / db-v${remoteInfo.db_compat_version ?? "?"}`
    : uiText("未读取到版本层级", "Versioned path not loaded", "バージョン階層は未取得です");

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
        <WebDavToggleCard
          title={uiText("同步总开关", "Sync Enabled", "同期の有効化")}
          description={settings.enabled
            ? uiText("已启用", "Enabled", "有効")
            : uiText("未启用", "Disabled", "無効")}
          enabled={settings.enabled}
          disabled={busy}
          onToggle={handleToggleEnabled}
        />

        <WebDavToggleCard
          title={uiText("定时自动同步", "Automatic Sync", "自動同期")}
          description={settings.auto_sync
            ? uiText("每 15 分钟尝试上传一次", "Attempt upload every 15 minutes", "15 分ごとにアップロードを試行")
            : uiText("仅手动同步", "Manual sync only", "手動同期のみ")}
          enabled={settings.auto_sync}
          disabled={busy}
          onToggle={handleToggleAutoSync}
        />

        <WebDavInfoCard
          title={uiText("最近同步", "Last Sync", "前回同期")}
          value={formatDateTime(settings.last_sync_at)}
          detail={remoteLayoutLabel}
        />
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
            onChange={handlePresetChange}
          >
            {WEBDAV_PRESETS.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.label}
              </option>
            ))}
          </select>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>
            {presetHint}
          </div>
        </div>

        {settingsFields.map((field) => (
          <WebDavTextField
            key={field.fieldKey}
            fieldKey={field.fieldKey}
            label={field.label}
            value={field.value}
            disabled={field.disabled}
            onValueChange={field.onValueChange}
            placeholder={field.placeholder}
            type={field.type}
          />
        ))}
      </div>

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
        <WebDavActionButton
          label={actionState === "saving"
            ? uiText("保存中...", "Saving...", "保存中...")
            : uiText("保存设置", "Save Settings", "設定を保存")}
          loading={actionState === "saving"}
          disabled={busy}
          icon={Save}
          variant="btn-primary"
          onClick={handleSaveClick}
        />
        <WebDavActionButton
          label={actionState === "testing"
            ? uiText("测试中...", "Testing...", "テスト中...")
            : uiText("测试连接", "Test Connection", "接続テスト")}
          loading={actionState === "testing"}
          disabled={busy}
          icon={Wifi}
          onClick={handleTestClick}
        />
        <WebDavActionButton
          label={actionState === "refreshing"
            ? uiText("刷新中...", "Refreshing...", "更新中...")
            : uiText("刷新远端", "Refresh Remote", "リモートを更新")}
          loading={actionState === "refreshing"}
          disabled={busy}
          icon={RefreshCw}
          onClick={handleRefreshRemoteClick}
        />
        <WebDavActionButton
          label={actionState === "uploading"
            ? uiText("上传中...", "Uploading...", "アップロード中...")
            : uiText("上传当前快照", "Upload Snapshot", "現在のスナップショットをアップロード")}
          loading={actionState === "uploading"}
          disabled={busy}
          icon={Upload}
          onClick={handleUploadClick}
        />
        <WebDavActionButton
          label={actionState === "downloading"
            ? uiText("恢复中...", "Restoring...", "復元中...")
            : uiText("从远端恢复", "Restore From Remote", "リモートから復元")}
          loading={actionState === "downloading"}
          disabled={busy || !remoteInfo?.exists}
          icon={Download}
          onClick={handleDownloadClick}
        />
        {remoteInfo?.remote_url && (
          <WebDavActionButton
            label={uiText("复制远端地址", "Copy Remote URL", "リモート URL をコピー")}
            loading={false}
            disabled={busy}
            icon={Copy}
            variant="btn-ghost"
            onClick={handleCopyRemoteUrl}
          />
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
        <WebDavInfoCard
          title={uiText("远端状态", "Remote Status", "リモート状態")}
          value={remoteStatusValue}
          detail={remoteStatusDetail}
          icon={remoteStatusIcon}
          iconColor={remoteStatusColor}
        />

        <WebDavInfoCard
          title={uiText("远端快照体积", "Remote Snapshot Size", "リモートスナップショットサイズ")}
          value={formatBytes(remoteInfo?.size_bytes ?? null)}
          detail={remoteSizeDetail}
          valueLarge
        />

        <WebDavInfoCard
          title={uiText("远端路径", "Remote Path", "リモートパス")}
          value={remoteInfo?.profile_path || "—"}
          detail={remotePathDetail}
          mono
        />
      </div>

      {remoteInfo?.snapshot_path && (
        <WebDavSnapshotDetails
          title={uiText("远端快照详情", "Remote Snapshot Details", "リモートスナップショット詳細")}
          snapshotLabel={uiText("快照文件", "Snapshot", "スナップショット")}
          snapshotPath={remoteInfo.snapshot_path}
          deviceLabel={uiText("设备名", "Device", "デバイス")}
          deviceName={remoteInfo.device_name || "—"}
          manifestLabel={uiText("Manifest 地址", "Manifest URL", "Manifest URL")}
          remoteUrl={remoteInfo.remote_url}
          copyTitle={uiText("复制 manifest 地址", "Copy manifest URL", "manifest URL をコピー")}
          canCopy={Boolean(remoteInfo.remote_url)}
          onCopy={handleCopyManifestUrl}
        />
      )}
    </div>
  );
}

export default memo(WebDavSyncSectionComponent);
