import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Cloud, Download, Loader2, RefreshCw, Save, Upload, Wifi } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import { Switch } from "./ui/switch";
import { Input } from "./ui/input";
import { Button } from "./ui/button";
import { useAppDialog } from "./AppDialogProvider";
import { cloudSettingsChanged, sameS3Account, sameS3BackupLocation } from "../lib/cloudSyncSettings";
import { BackupEncryptionField } from "./cloud-sync/BackupEncryptionField";
import { CloudUploadStatus } from "./cloud-sync/CloudUploadStatus";
import { confirmCloudUpload, type CloudUploadReview } from "../lib/cloudUploadReview";
import { refreshBackupRestoreState } from "../lib/backupRestoreState";
import { useCloudOperations, useCloudWindowRefresh, type CloudOperation } from "./cloud-sync/useCloudOperations";
import {
  backupPasswordAvailable,
  EMPTY_BACKUP_ENCRYPTION,
  maskBackupEncryption,
  type BackupEncryptionSettings,
} from "../lib/backupEncryption";

interface S3SyncSettings {
  enabled: boolean;
  endpoint: string;
  region: string;
  bucket: string;
  accessKeyId: string;
  secretAccessKey: string;
  hasSecretAccessKey: boolean;
  remoteRoot: string;
  profile: string;
  autoSync: boolean;
  backupEncryption: BackupEncryptionSettings;
  lastSyncAt: string | null;
  lastError: string | null;
}

interface S3RemoteInfo {
  exists: boolean;
  remoteUrl: string;
  snapshotPath: string | null;
  updatedAt: string | null;
  sizeBytes: number | null;
  compatible: boolean;
  encrypted?: boolean;
  uploadReview?: CloudUploadReview | null;
  profilePath: string;
}

const DEFAULT_SETTINGS: S3SyncSettings = {
  enabled: false,
  endpoint: "",
  region: "us-east-1",
  bucket: "",
  accessKeyId: "",
  secretAccessKey: "",
  hasSecretAccessKey: false,
  remoteRoot: "cchub-sync",
  profile: "default",
  autoSync: false,
  backupEncryption: EMPTY_BACKUP_ENCRYPTION,
  lastSyncAt: null,
  lastError: null,
};

type Action =
  | "idle"
  | "loading"
  | "saving"
  | "testing"
  | "refreshing"
  | "reviewing"
  | "confirming"
  | "uploading"
  | "downloading";

export default function S3SyncSection({ onRestored }: { onRestored?: () => Promise<void> }) {
  const appDialog = useAppDialog();
  const locale = getLocale();
  const [settings, setSettings] = useState<S3SyncSettings>(DEFAULT_SETTINGS);
  const [savedSettings, setSavedSettings] = useState<S3SyncSettings | null>(null);
  const [remote, setRemote] = useState<S3RemoteInfo | null>(null);
  const [action, setAction] = useState<Action>("loading");
  const [secretTouched, setSecretTouched] = useState(false);
  const operations = useCloudOperations();
  const loadedRef = useRef<S3SyncSettings | null>(null);
  const draftRef = useRef(false);
  const text = useCallback((zh: string, en: string) => (locale === "zh" ? zh : en), [locale]);
  const busy = action !== "idle";
  const dirty =
    secretTouched ||
    settings.backupEncryption.passphraseTouched ||
    cloudSettingsChanged(settings, savedSettings, [
      "enabled",
      "endpoint",
      "region",
      "bucket",
      "accessKeyId",
      "secretAccessKey",
      "remoteRoot",
      "profile",
      "autoSync",
    ]);
  draftRef.current = dirty;
  const savedAccount = savedSettings !== null && sameS3Account(settings, savedSettings);
  const sameBackup = savedSettings !== null && sameS3BackupLocation(settings, savedSettings);
  const passwordAvailable = backupPasswordAvailable(settings.backupEncryption, sameBackup);
  const remoteActionsDisabled = busy || dirty || !savedSettings || !savedSettings.enabled;

  const applyLoaded = useCallback((response: S3SyncSettings) => {
    const loaded = {
      ...DEFAULT_SETTINGS,
      ...response,
      secretAccessKey: "",
      backupEncryption: maskBackupEncryption(response.backupEncryption),
    };
    if (
      !loaded.enabled ||
      !loadedRef.current ||
      !sameS3BackupLocation(loaded, loadedRef.current) ||
      (loaded.region.trim() || "us-east-1") !== (loadedRef.current.region.trim() || "us-east-1")
    )
      setRemote(null);
    loadedRef.current = loaded;
    setSettings(loaded);
    setSavedSettings(loaded);
    setSecretTouched(false);
  }, []);

  const load = useCallback(
    async (silent = false, owned?: CloudOperation) => {
      if (!owned && silent && draftRef.current) return false;
      const operation = owned ?? operations.beginRead();
      if (!operation) return false;
      if (!silent && !owned) setAction("loading");
      try {
        const response = await invoke<S3SyncSettings>("get_s3_sync_settings");
        if (!operation.current() || (!owned && silent && draftRef.current)) return false;
        applyLoaded(response);
        return true;
      } catch (error) {
        if (!silent && operation.current())
          showToast("error", `${getLocale() === "zh" ? "读取 S3 设置失败" : "Failed to load S3 settings"}: ${error}`);
        return false;
      } finally {
        if (!owned && operation.current()) {
          if (!silent) setAction("idle");
          operation.finish();
        }
      }
    },
    [applyLoaded, operations],
  );

  useCloudWindowRefresh(() => {
    if (!busy && !dirty) void load(true);
  });

  useEffect(() => {
    void load();
  }, [load]);

  const update = <K extends keyof S3SyncSettings>(key: K, value: S3SyncSettings[K]) => {
    operations.invalidateReads();
    setSettings((current) => ({ ...current, [key]: value }));
  };

  const save = async () => {
    if (busy || !savedSettings) return;
    const operation = operations.beginAction();
    if (!operation) return;
    setAction("saving");
    try {
      const saved = await invoke<S3SyncSettings>("set_s3_sync_settings", { settings, secretTouched });
      if (!operation.current()) return;
      applyLoaded(saved);
      setRemote(null);
      showToast("success", text("S3 设置已保存", "S3 settings saved"));
    } catch (error) {
      if (operation.current())
        showToast("error", `${text("保存 S3 设置失败", "Failed to save S3 settings")}: ${error}`);
    } finally {
      if (operation.current()) {
        setAction("idle");
        operation.finish();
      }
    }
  };

  const test = async () => {
    if (busy || !savedSettings) return;
    const operation = operations.beginAction();
    if (!operation) return;
    setAction("testing");
    try {
      await invoke("s3_test_connection", { settings, preserveEmptySecret: !secretTouched });
      if (!operation.current()) return;
      showToast("success", text("S3 连接成功", "S3 connection succeeded"));
    } catch (error) {
      if (operation.current()) showToast("error", `${text("S3 连接失败", "S3 connection failed")}: ${error}`);
    } finally {
      if (operation.current()) {
        setAction("idle");
        operation.finish();
      }
    }
  };

  const refreshRemote = async () => {
    if (remoteActionsDisabled) return;
    const operation = operations.beginAction();
    if (!operation) return;
    setAction("refreshing");
    try {
      const latest = await invoke<S3RemoteInfo>("s3_sync_fetch_remote_info");
      if (!operation.current()) return;
      setRemote(latest);
    } catch (error) {
      if (operation.current())
        showToast("error", `${text("读取远端状态失败", "Failed to read remote status")}: ${error}`);
    } finally {
      if (operation.current()) {
        setAction("idle");
        operation.finish();
      }
    }
  };

  const upload = async () => {
    if (remoteActionsDisabled || !passwordAvailable) return;
    const operation = operations.beginAction();
    if (!operation) return;
    setAction("reviewing");
    try {
      const latest = await invoke<S3RemoteInfo>("s3_sync_fetch_remote_info");
      if (!operation.current()) return;
      setRemote(latest);
      if (!latest.compatible)
        throw new Error(
          text(
            "远端备份无效或不兼容，请更换备份位置后重试。",
            "Remote backup is invalid or incompatible. Choose another backup location.",
          ),
        );
      const reviewedRevision = await confirmCloudUpload(
        latest.uploadReview,
        latest.exists,
        latest.updatedAt,
        appDialog.confirm,
        text,
      );
      if (reviewedRevision === null || !operation.current()) return;
      setAction("uploading");
      const info = await invoke<S3RemoteInfo>("s3_sync_upload", { reviewedRevision });
      if (!operation.current()) return;
      setRemote(info);
      await load(true, operation);
      if (!operation.current()) return;
      showToast("success", text("快照已上传到 S3", "Snapshot uploaded to S3"));
    } catch (error) {
      if (operation.current()) showToast("error", `${text("S3 上传失败", "S3 upload failed")}: ${error}`);
    } finally {
      if (operation.current()) {
        setAction("idle");
        operation.finish();
      }
    }
  };

  const download = async () => {
    if (remoteActionsDisabled || !remote?.exists || !remote.compatible || (remote.encrypted && !passwordAvailable))
      return;
    const allowPlaintext = !remote.encrypted;
    const operation = operations.beginAction();
    if (!operation) return;
    setAction("confirming");
    try {
      const confirmed = await appDialog.confirm({
        title: text("从 S3 恢复", "Restore from S3"),
        message: allowPlaintext
          ? text(
              "这份旧备份未加密。恢复会替换配置数据并保留本机云账号、工具目录和代理设置。项目文件需确认本机路径后迁移，请先保存本地工作。",
              "This older backup is unencrypted. Restoring replaces configuration data while keeping this device's cloud accounts, tool directories and proxy settings. Project files require local path mapping. Save local work first.",
            )
          : text(
              "恢复会替换配置数据并保留本机云账号、工具目录和代理设置。项目文件需确认本机路径后迁移，请先保存本地工作。",
              "Restoring replaces configuration data while keeping this device's cloud accounts, tool directories and proxy settings. Project files require local path mapping. Save local work first.",
            ),
        confirmText: text("继续恢复", "Restore"),
        cancelText: text("取消", "Cancel"),
        tone: "warning",
      });
      if (!confirmed || !operation.current()) return;
      setAction("downloading");
      await invoke<string>("s3_sync_download", { allowPlaintext });
      const refreshed = await refreshBackupRestoreState(operation.current() ? onRestored : undefined);
      if (!operation.current()) return;
      const loaded = await load(true, operation);
      if (!operation.current()) return;
      showToast(
        refreshed && loaded ? "success" : "info",
        refreshed && loaded
          ? text("已从 S3 恢复快照", "Snapshot restored from S3")
          : text(
              "快照已恢复，部分页面未刷新，请重新打开相关页面。",
              "Snapshot restored. Some views could not refresh; reopen them.",
            ),
      );
    } catch (error) {
      if (operation.current()) showToast("error", `${text("S3 恢复失败", "S3 restore failed")}: ${error}`);
    } finally {
      if (operation.current()) {
        setAction("idle");
        operation.finish();
      }
    }
  };

  const fields = useMemo(
    () =>
      [
        [
          "endpoint",
          text("Endpoint（留空使用 AWS S3）", "Endpoint (leave blank for AWS S3)"),
          "https://s3.example.com",
        ],
        ["region", text("区域", "Region"), "us-east-1"],
        ["bucket", text("Bucket", "Bucket"), "my-cchub-backups"],
        ["accessKeyId", text("Access Key ID", "Access Key ID"), ""],
        ["remoteRoot", text("远端根目录", "Remote root"), "cchub-sync"],
        ["profile", text("Profile", "Profile"), "default"],
      ] as const,
    [text],
  );

  return (
    <div className="section-card">
      <div className="section-card-title">
        <Cloud size={17} style={{ color: "var(--text-secondary)" }} />
        {text("S3 / MinIO 云同步", "S3 / MinIO Cloud Sync")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
        {text(
          "将配置加密备份到 S3 或兼容的对象存储，在其他设备上恢复。登录密钥和备份密码保存在系统密钥环中；恢复前会验证备份完整性。",
          "Back up encrypted configuration to S3 or compatible storage and restore it on another device. Login keys and backup passwords stay in the OS keyring. Backups are checked for integrity before restoring.",
        )}
      </p>
      {dirty && (
        <p role="status" className="mb-4 text-xs text-[var(--text-secondary)]">
          {text(
            "有未保存的修改，请保存后再读取、上传或恢复远端备份。",
            "Save your changes before reading, uploading, or restoring remote backups.",
          )}
        </p>
      )}
      {!savedSettings && !busy && (
        <Button variant="secondary" onClick={() => void load()}>
          {text("重新读取设置", "Retry loading settings")}
        </Button>
      )}
      <div style={{ display: "flex", gap: 10, flexWrap: "wrap", marginBottom: 14 }}>
        <Toggle
          label={text("启用同步", "Enable sync")}
          value={settings.enabled}
          disabled={busy}
          onChange={(value) => update("enabled", value)}
        />
        <Toggle
          label={text("每 15 分钟自动上传", "Upload every 15 minutes")}
          value={settings.autoSync}
          disabled={busy || (!settings.autoSync && !passwordAvailable)}
          onChange={(value) => update("autoSync", value)}
        />
      </div>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
          gap: 10,
          marginBottom: 14,
        }}
      >
        {fields.map(([key, label, placeholder]) => (
          <label key={key} style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            <span style={{ display: "block", marginBottom: 5 }}>{label}</span>
            <Input
              value={String(settings[key])}
              placeholder={placeholder}
              disabled={busy}
              onChange={(event) => update(key, event.target.value)}
            />
          </label>
        ))}
        <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          <span style={{ display: "block", marginBottom: 5 }}>{text("Secret Access Key", "Secret Access Key")}</span>
          <Input
            type="password"
            value={settings.secretAccessKey}
            placeholder={
              settings.hasSecretAccessKey && !secretTouched && savedAccount
                ? text("已保存，留空保持不变", "Saved; leave blank to keep")
                : ""
            }
            disabled={busy}
            onChange={(event) => {
              setSecretTouched(true);
              update("secretAccessKey", event.target.value);
            }}
          />
        </label>
        <BackupEncryptionField
          value={settings.backupEncryption}
          sameLocation={sameBackup}
          disabled={busy}
          text={text}
          onChange={(value) => update("backupEncryption", value)}
        />
      </div>
      <CloudUploadStatus review={remote?.uploadReview} text={text} />
      {!passwordAvailable && savedSettings && (
        <p className="mb-3 text-xs text-[var(--text-secondary)]">
          {text(
            "请设置并保存备份密码以启用上传和加密备份恢复。",
            "Set and save a backup password to upload or restore encrypted backups.",
          )}
        </p>
      )}
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 14 }}>
        <ActionButton
          icon={Save}
          label={action === "saving" ? text("保存中...", "Saving...") : text("保存设置", "Save settings")}
          disabled={busy || !savedSettings}
          loading={action === "saving"}
          onClick={() => void save()}
        />
        <ActionButton
          icon={Wifi}
          label={action === "testing" ? text("测试中...", "Testing...") : text("测试连接", "Test connection")}
          disabled={busy || !savedSettings}
          loading={action === "testing"}
          onClick={() => void test()}
        />
        <ActionButton
          icon={RefreshCw}
          label={action === "refreshing" ? text("刷新中...", "Refreshing...") : text("刷新远端", "Refresh remote")}
          disabled={remoteActionsDisabled}
          loading={action === "refreshing"}
          onClick={() => void refreshRemote()}
        />
        <ActionButton
          icon={Upload}
          label={
            action === "reviewing"
              ? text("检查备份...", "Checking backup...")
              : action === "uploading"
                ? text("上传中...", "Uploading...")
                : text("上传快照", "Upload snapshot")
          }
          disabled={remoteActionsDisabled || !passwordAvailable}
          loading={action === "reviewing" || action === "uploading"}
          onClick={() => void upload()}
        />
        <ActionButton
          icon={Download}
          label={
            action === "confirming"
              ? text("等待确认", "Waiting for confirmation")
              : action === "downloading"
                ? text("恢复中...", "Restoring...")
                : text("从远端恢复", "Restore remote")
          }
          disabled={
            remoteActionsDisabled || !remote?.exists || !remote.compatible || (!!remote.encrypted && !passwordAvailable)
          }
          loading={action === "downloading"}
          onClick={() => void download()}
        />
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 10 }}>
        <InfoCard
          label={text("远端状态", "Remote status")}
          value={
            remote?.exists
              ? remote.compatible
                ? remote.encrypted
                  ? text("加密备份", "Encrypted backup")
                  : text("旧备份，未加密", "Legacy, unencrypted backup")
                : text("备份信息无效或不兼容", "Invalid or incompatible backup")
              : remote
                ? text("远端暂无备份", "No remote backup")
                : text("尚未读取", "Not loaded")
          }
        />
        <InfoCard label={text("远端路径", "Remote path")} value={remote?.profilePath || "—"} mono />
        <InfoCard label={text("最近同步", "Last sync")} value={settings.lastSyncAt || "—"} />
      </div>
      {settings.lastError && (
        <div style={{ marginTop: 12, color: "var(--danger)", fontSize: 12 }}>{settings.lastError}</div>
      )}
    </div>
  );
}

function Toggle({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string;
  value: boolean;
  disabled: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="inline-flex min-h-8 items-center gap-2.5 rounded-lg border border-[var(--border-default)] bg-[var(--control-background)] px-2.5 text-xs font-medium text-[var(--text-secondary)]">
      <Switch checked={value} disabled={disabled} onCheckedChange={onChange} aria-label={label} />
      <span>{label}</span>
    </label>
  );
}

function ActionButton({
  icon: Icon,
  label,
  disabled,
  loading,
  onClick,
}: {
  icon: typeof Save;
  label: string;
  disabled: boolean;
  loading: boolean;
  onClick: () => void;
}) {
  return (
    <Button variant="secondary" type="button" disabled={disabled} onClick={onClick} aria-busy={loading}>
      {loading ? (
        <Loader2 size={14} aria-hidden="true" className="animate-spin" />
      ) : (
        <Icon size={14} aria-hidden="true" />
      )}
      {label}
    </Button>
  );
}

function InfoCard({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ padding: "10px 12px", border: "1px solid var(--border-default)", borderRadius: 8 }}>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{label}</div>
      <div
        style={{
          fontSize: 12,
          fontFamily: mono ? "var(--font-mono)" : undefined,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={value}
      >
        {value}
      </div>
    </div>
  );
}
