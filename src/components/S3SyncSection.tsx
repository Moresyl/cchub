import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle, Cloud, Download, Loader2, RefreshCw, Save, Upload, Wifi } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";

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
  const locale = getLocale();
  const [settings, setSettings] = useState<S3SyncSettings>(DEFAULT_SETTINGS);
  const [remote, setRemote] = useState<S3RemoteInfo | null>(null);
  const [action, setAction] = useState<Action>("loading");
  const [secretTouched, setSecretTouched] = useState(false);
  const text = useCallback((zh: string, en: string) => (locale === "zh" ? zh : en), [locale]);
  const busy = action !== "idle";

  const load = useCallback(async () => {
    setAction("loading");
    try {
      setSettings(await invoke<S3SyncSettings>("get_s3_sync_settings"));
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
    setAction("saving");
    try {
      const saved = await invoke<S3SyncSettings>("set_s3_sync_settings", { settings, secretTouched });
      setSettings(saved);
      setSecretTouched(false);
      showToast("success", text("S3 设置已保存", "S3 settings saved"));
    } catch (error) {
      showToast("error", `${text("保存 S3 设置失败", "Failed to save S3 settings")}: ${error}`);
    } finally {
      setAction("idle");
    }
  };

  const test = async () => {
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
          "使用 AWS Signature V4 连接 AWS S3 或兼容对象存储。密钥只保存到系统密钥环，快照上传前会生成校验清单，恢复时会验证大小、版本和 SHA-256。",
          "Connect to AWS S3 or compatible object storage with AWS Signature V4. Secrets stay in the OS keyring; snapshots are restored only after manifest, size, version, and SHA-256 checks.",
        )}
      </p>
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
            <input
              className="input"
              value={String(settings[key])}
              placeholder={placeholder}
              disabled={busy}
              onChange={(event) => update(key, event.target.value)}
            />
          </label>
        ))}
        <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          <span style={{ display: "block", marginBottom: 5 }}>{text("Secret Access Key", "Secret Access Key")}</span>
          <input
            className="input"
            type="password"
            value={settings.secretAccessKey}
            placeholder={settings.hasSecretAccessKey ? text("已保存，留空保持不变", "Saved; leave blank to keep") : ""}
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
          disabled={busy}
          onClick={() => void save()}
        />
        <ActionButton
          icon={Wifi}
          label={action === "testing" ? text("测试中...", "Testing...") : text("测试连接", "Test connection")}
          disabled={busy}
          onClick={() => void test()}
        />
        <ActionButton
          icon={RefreshCw}
          label={action === "refreshing" ? text("刷新中...", "Refreshing...") : text("刷新远端", "Refresh remote")}
          disabled={busy}
          onClick={() => void refreshRemote()}
        />
        <ActionButton
          icon={Upload}
          label={action === "uploading" ? text("上传中...", "Uploading...") : text("上传快照", "Upload snapshot")}
          disabled={busy || !settings.enabled}
          onClick={() => void upload()}
        />
        <ActionButton
          icon={Download}
          label={action === "downloading" ? text("恢复中...", "Restoring...") : text("从远端恢复", "Restore remote")}
          disabled={busy || !settings.enabled || !remote?.exists || !remote.compatible}
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
    <button
      className="btn btn-secondary btn-sm"
      type="button"
      disabled={disabled}
      aria-pressed={value}
      onClick={() => onChange(!value)}
    >
      <CheckCircle size={13} style={{ color: value ? "var(--success)" : "var(--text-muted)" }} />
      {label}
    </button>
  );
}

function ActionButton({
  icon: Icon,
  label,
  disabled,
  onClick,
}: {
  icon: typeof Save;
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button className="btn btn-secondary btn-sm" type="button" disabled={disabled} onClick={onClick}>
      {disabled ? <Loader2 size={13} className="animate-spin" /> : <Icon size={13} />}
      {label}
    </button>
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
