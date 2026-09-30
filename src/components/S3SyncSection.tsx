import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Cloud, Download, Loader2, RefreshCw, Save, Upload, Wifi } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import { Switch } from "./ui/switch";
import { Input } from "./ui/input";
import { Button } from "./ui/button";
import { useAppDialog } from "./AppDialogProvider";
import { cloudSettingsChanged, sameS3Account } from "../lib/cloudSyncSettings";

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
  lastSyncAt: null,
  lastError: null,
};

type Action = "idle" | "loading" | "saving" | "testing" | "refreshing" | "uploading" | "downloading";

export default function S3SyncSection() {
  const appDialog = useAppDialog();
  const locale = getLocale();
  const [settings, setSettings] = useState<S3SyncSettings>(DEFAULT_SETTINGS);
  const [savedSettings, setSavedSettings] = useState<S3SyncSettings | null>(null);
  const [remote, setRemote] = useState<S3RemoteInfo | null>(null);
  const [action, setAction] = useState<Action>("loading");
  const [secretTouched, setSecretTouched] = useState(false);
  const text = useCallback((zh: string, en: string) => (locale === "zh" ? zh : en), [locale]);
  const busy = action !== "idle";
  const dirty =
    secretTouched ||
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
  const savedAccount = savedSettings !== null && sameS3Account(settings, savedSettings);
  const remoteActionsDisabled = busy || dirty || !savedSettings || !savedSettings.enabled;

  const load = useCallback(async () => {
    setAction("loading");
    try {
      const loaded = {
        ...DEFAULT_SETTINGS,
        ...(await invoke<S3SyncSettings>("get_s3_sync_settings")),
        secretAccessKey: "",
      };
      setSettings(loaded);
      setSavedSettings(loaded);
      setSecretTouched(false);
    } catch (error) {
      showToast("error", `${text("读取 S3 设置失败", "Failed to load S3 settings")}: ${error}`);
    } finally {
      setAction("idle");
    }
  }, [text]);

  useEffect(() => {
    void load();
  }, [load]);

  const update = <K extends keyof S3SyncSettings>(key: K, value: S3SyncSettings[K]) => {
    setSettings((current) => ({ ...current, [key]: value }));
  };

  const save = async () => {
    if (busy || !savedSettings) return;
    setAction("saving");
    try {
      const saved = await invoke<S3SyncSettings>("set_s3_sync_settings", { settings, secretTouched });
      const masked = { ...DEFAULT_SETTINGS, ...saved, secretAccessKey: "" };
      setSettings(masked);
      setSavedSettings(masked);
      setRemote(null);
      setSecretTouched(false);
      showToast("success", text("S3 设置已保存", "S3 settings saved"));
    } catch (error) {
      showToast("error", `${text("保存 S3 设置失败", "Failed to save S3 settings")}: ${error}`);
    } finally {
      setAction("idle");
    }
  };

  const test = async () => {
    if (busy || !savedSettings) return;
    setAction("testing");
    try {
      await invoke("s3_test_connection", { settings, preserveEmptySecret: !secretTouched });
      showToast("success", text("S3 连接成功", "S3 connection succeeded"));
    } catch (error) {
      showToast("error", `${text("S3 连接失败", "S3 connection failed")}: ${error}`);
    } finally {
      setAction("idle");
    }
  };

  const refreshRemote = async () => {
    if (remoteActionsDisabled) return;
    setAction("refreshing");
    try {
      setRemote(await invoke<S3RemoteInfo>("s3_sync_fetch_remote_info"));
    } catch (error) {
      showToast("error", `${text("读取远端状态失败", "Failed to read remote status")}: ${error}`);
    } finally {
      setAction("idle");
    }
  };

  const upload = async () => {
    if (remoteActionsDisabled) return;
    setAction("uploading");
    try {
      setRemote(await invoke<S3RemoteInfo>("s3_sync_upload"));
      await load();
      showToast("success", text("快照已上传到 S3", "Snapshot uploaded to S3"));
    } catch (error) {
      showToast("error", `${text("S3 上传失败", "S3 upload failed")}: ${error}`);
    } finally {
      setAction("idle");
    }
  };

  const download = async () => {
    if (remoteActionsDisabled || !remote?.exists || !remote.compatible) return;
    const confirmed = await appDialog.confirm({
      title: text("从 S3 恢复", "Restore from S3"),
      message: text(
        "远端备份会覆盖当前数据库，请确认本地工作已保存。",
        "The remote backup will replace the current database. Make sure local work is saved.",
      ),
      confirmText: text("继续恢复", "Restore"),
      cancelText: text("取消", "Cancel"),
      tone: "warning",
    });
    if (!confirmed) return;
    setAction("downloading");
    try {
      await invoke<string>("s3_sync_download");
      await load();
      showToast("success", text("已从 S3 恢复快照", "Snapshot restored from S3"));
    } catch (error) {
      showToast("error", `${text("S3 恢复失败", "S3 restore failed")}: ${error}`);
    } finally {
      setAction("idle");
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
          "将配置备份到 S3 或兼容的对象存储，在其他设备上恢复。密钥保存在系统密钥环中，仅用于对应的服务器与账号；恢复前会验证备份完整性。",
          "Back up your configuration to S3 or compatible storage and restore it on another device. Keys stay in the OS keyring and belong to the corresponding server and account. Backups are checked for integrity before restoring.",
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
          disabled={busy}
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
      </div>
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
          label={action === "uploading" ? text("上传中...", "Uploading...") : text("上传快照", "Upload snapshot")}
          disabled={remoteActionsDisabled}
          loading={action === "uploading"}
          onClick={() => void upload()}
        />
        <ActionButton
          icon={Download}
          label={action === "downloading" ? text("恢复中...", "Restoring...") : text("从远端恢复", "Restore remote")}
          disabled={remoteActionsDisabled || !remote?.exists || !remote.compatible}
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
                ? text("可恢复快照", "Compatible snapshot")
                : text("版本不兼容", "Incompatible snapshot")
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
