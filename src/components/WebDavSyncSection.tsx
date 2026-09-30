/* eslint-disable react-hooks/exhaustive-deps */
import { memo, startTransition, useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AlertCircle, CheckCircle, Copy, Download, Link2, RefreshCw, Save, Upload, Wifi } from "lucide-react";
import { getLocale, t } from "../lib/i18n";
import { showToast } from "./Toast";
import { useSetWebDavSyncSettingsMutation } from "../hooks/mutations";
import { useAppDialog } from "./AppDialogProvider";
import { SimpleSelect } from "./ui/simple-select";
import { cloudSettingsChanged, sameWebDavAccount, sameWebDavBackupLocation } from "../lib/cloudSyncSettings";
import { Button } from "./ui/button";
import { BackupEncryptionField } from "./cloud-sync/BackupEncryptionField";
import { CloudUploadStatus } from "./cloud-sync/CloudUploadStatus";
import { confirmCloudUpload } from "../lib/cloudUploadReview";
import { backupPasswordAvailable, maskBackupEncryption } from "../lib/backupEncryption";
import { refreshBackupRestoreState } from "../lib/backupRestoreState";

import {
  EMPTY_SETTINGS,
  WEBDAV_PRESETS,
  WebDavActionButton,
  WebDavInfoCard,
  WebDavSnapshotDetails,
  WebDavTextField,
  WebDavToggleCard,
  detectPreset,
  formatBytes,
  formatDateTime,
  type ActionState,
  type WebDavRemoteInfo,
  type WebDavSyncEvent,
  type WebDavSyncSettings,
  type WebDavTextFieldKey,
  type WebDavTextFieldProps,
} from "./webdav-sync/parts";

function WebDavSyncSectionComponent({ onRestored }: { onRestored?: () => Promise<void> }) {
  const appDialog = useAppDialog();
  const loc = getLocale();
  const i = t();
  const setWebDavSyncSettingsMutation = useSetWebDavSyncSettingsMutation<WebDavSyncSettings>();
  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      loc === "zh" ? zhText : loc === "ja" ? (jaText ?? enText) : enText,
    [loc],
  );

  const [settings, setSettings] = useState<WebDavSyncSettings>(EMPTY_SETTINGS);
  const [savedSettings, setSavedSettings] = useState<WebDavSyncSettings | null>(null);
  const [remoteInfo, setRemoteInfo] = useState<WebDavRemoteInfo | null>(null);
  const [presetId, setPresetId] = useState("custom");
  const [passwordTouched, setPasswordTouched] = useState(false);
  const [actionState, setActionState] = useState<ActionState>("loading");
  const dirty =
    passwordTouched ||
    settings.backup_encryption.passphraseTouched ||
    cloudSettingsChanged(settings, savedSettings, [
      "enabled",
      "base_url",
      "username",
      "password",
      "remote_root",
      "profile",
      "auto_sync",
    ]);
  const draftRef = useRef(false);
  draftRef.current = dirty;
  const savedAccount = savedSettings !== null && sameWebDavAccount(settings, savedSettings);
  const sameBackup = savedSettings !== null && sameWebDavBackupLocation(settings, savedSettings);
  const passwordAvailable = backupPasswordAvailable(settings.backup_encryption, sameBackup);
  const remoteActionsDisabled = actionState !== "idle" || dirty || !savedSettings || !savedSettings.enabled;

  const applyLoadedState = useCallback((nextSettings: WebDavSyncSettings, nextRemoteInfo: WebDavRemoteInfo | null) => {
    startTransition(() => {
      const masked = {
        ...EMPTY_SETTINGS,
        ...nextSettings,
        password: "",
        backup_encryption: maskBackupEncryption(nextSettings.backup_encryption),
      };
      setSettings(masked);
      setSavedSettings(masked);
      setRemoteInfo(nextRemoteInfo);
      setPresetId(detectPreset(nextSettings.base_url));
      setPasswordTouched(false);
    });
  }, []);

  const loadState = useCallback(
    async (silent = false) => {
      if (silent && draftRef.current) return;
      if (!silent) {
        setActionState("loading");
      }
      try {
        const [nextSettings, nextRemoteInfo] = await Promise.all([
          invoke<WebDavSyncSettings>("get_webdav_sync_settings"),
          invoke<WebDavRemoteInfo>("webdav_sync_fetch_remote_info").catch(() => null),
        ]);
        if (!silent || !draftRef.current) applyLoadedState(nextSettings, nextRemoteInfo);
      } catch (error) {
        if (!silent) {
          showToast("error", String(error));
        }
      } finally {
        if (!silent) {
          setActionState("idle");
        }
      }
    },
    [applyLoadedState],
  );

  const handleSyncEvent = useEffectEvent((payload: WebDavSyncEvent) => {
    if (actionState !== "idle") return;
    void loadState(true);
    if (payload.status === "success") {
      showToast(
        "success",
        uiText("WebDAV 自动同步已完成", "Automatic WebDAV sync completed", "WebDAV 自動同期が完了しました"),
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
    const unlisten = listen<WebDavSyncEvent>("webdav-sync-status-updated", (event) => handleSyncEvent(event.payload));
    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, [loadState]);

  const busy = actionState !== "idle";
  const activePreset = useMemo(() => WEBDAV_PRESETS.find((preset) => preset.id === presetId), [presetId]);

  const updateSettings = useCallback(<K extends keyof WebDavSyncSettings>(key: K, value: WebDavSyncSettings[K]) => {
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

  const handleCopy = useCallback(
    async (value: string, label: string) => {
      if (!value) return;
      try {
        await navigator.clipboard.writeText(value);
        showToast("success", i.settings.copied.replace("{label}", label));
      } catch (error) {
        showToast("error", String(error));
      }
    },
    [i.settings.copied],
  );

  const handleSave = useCallback(async () => {
    setActionState("saving");
    try {
      const saved = await setWebDavSyncSettingsMutation.mutateAsync({
        settings,
        passwordTouched,
      });
      applyLoadedState(saved, null);
      showToast("success", uiText("WebDAV 设置已保存", "WebDAV settings saved", "WebDAV 設定を保存しました"));
      const nextRemoteInfo = await invoke<WebDavRemoteInfo>("webdav_sync_fetch_remote_info").catch((error) => {
        console.warn("Failed to refresh WebDAV remote info after saving settings", error);
        return null;
      });
      startTransition(() => {
        setRemoteInfo(nextRemoteInfo);
      });
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }, [applyLoadedState, passwordTouched, remoteInfo, setWebDavSyncSettingsMutation, settings, uiText]);

  const handleTest = useCallback(async () => {
    setActionState("testing");
    try {
      await invoke("webdav_test_connection", {
        settings,
        preserveEmptyPassword: !passwordTouched,
      });
      showToast("success", uiText("WebDAV 连接成功", "WebDAV connection succeeded", "WebDAV 接続に成功しました"));
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }, [passwordTouched, settings, uiText]);

  const refreshRemoteInfo = useCallback(async () => {
    if (remoteActionsDisabled) return;
    setActionState("refreshing");
    try {
      const nextRemoteInfo = await invoke<WebDavRemoteInfo>("webdav_sync_fetch_remote_info");
      startTransition(() => {
        setRemoteInfo(nextRemoteInfo);
      });
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }, [remoteActionsDisabled]);

  const handleUpload = useCallback(async () => {
    if (remoteActionsDisabled || !passwordAvailable) return;
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
    setActionState("reviewing");
    try {
      const latest = await invoke<WebDavRemoteInfo>("webdav_sync_fetch_remote_info");
      setRemoteInfo(latest);
      if (!latest.compatible)
        throw new Error(
          uiText(
            "远端备份无效或不兼容，请更换备份位置后重试。",
            "Remote backup is invalid or incompatible. Choose another backup location.",
          ),
        );
      const reviewedRevision = await confirmCloudUpload(
        latest.upload_review,
        latest.exists,
        latest.updated_at,
        appDialog.confirm,
        uiText,
      );
      if (reviewedRevision === null) return;
      setActionState("uploading");
      const info = await invoke<WebDavRemoteInfo>("webdav_sync_upload", { reviewedRevision });
      startTransition(() => {
        setRemoteInfo(info);
      });
      await loadState(true);
      showToast(
        "success",
        uiText(
          "已上传加密备份到 WebDAV",
          "Uploaded the encrypted backup to WebDAV",
          "暗号化バックアップを WebDAV にアップロードしました",
        ),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }, [appDialog, loadState, passwordAvailable, remoteActionsDisabled, settings.enabled, uiText]);

  const handleDownload = useCallback(async () => {
    if (
      remoteActionsDisabled ||
      !remoteInfo?.exists ||
      !remoteInfo.compatible ||
      (remoteInfo.encrypted && !passwordAvailable)
    )
      return;
    const allowPlaintext = !remoteInfo.encrypted;
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
    const confirmed = await appDialog.confirm({
      title: uiText("从 WebDAV 恢复", "Restore from WebDAV", "WebDAV から復元"),
      message: allowPlaintext
        ? uiText(
            "这份旧备份未加密。恢复会替换配置数据并保留本机云账号、工具目录和代理设置。项目文件需确认本机路径后迁移，请先保存本地工作。",
            "This older backup is unencrypted. Restoring replaces configuration data while keeping this device's cloud accounts, tool directories and proxy settings. Project files require local path mapping. Save local work first.",
            "この旧バックアップは暗号化されていません。設定データを復元し、この端末のクラウドアカウント、ツールの保存先、プロキシ設定は保持します。プロジェクトファイルには保存先の指定が必要です。作業を保存してください。",
          )
        : uiText(
            "恢复会替换配置数据并保留本机云账号、工具目录和代理设置。项目文件需确认本机路径后迁移，请先保存本地工作。",
            "Restoring replaces configuration data while keeping this device's cloud accounts, tool directories and proxy settings. Project files require local path mapping. Save local work first.",
            "設定データを復元し、この端末のクラウドアカウント、ツールの保存先、プロキシ設定は保持します。プロジェクトファイルには保存先の指定が必要です。作業を保存してください。",
          ),
      confirmText: uiText("继续恢复", "Restore", "復元を続行"),
      cancelText: uiText("取消", "Cancel", "キャンセル"),
      tone: "warning",
    });
    if (!confirmed) {
      return;
    }
    setActionState("downloading");
    try {
      await invoke<string>("webdav_sync_download", { allowPlaintext });
      const refreshed = await refreshBackupRestoreState(onRestored);
      await loadState(true);
      showToast(
        refreshed ? "success" : "info",
        refreshed
          ? uiText("已从 WebDAV 恢复快照", "Snapshot restored from WebDAV", "WebDAV のスナップショットを復元しました")
          : uiText(
              "快照已恢复，部分页面未刷新，请重新打开相关页面。",
              "Snapshot restored. Some views could not refresh; reopen them.",
              "復元は完了しました。一部の画面を再度開いてください。",
            ),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setActionState("idle");
    }
  }, [
    appDialog,
    loadState,
    onRestored,
    passwordAvailable,
    remoteActionsDisabled,
    remoteInfo,
    settings.enabled,
    uiText,
  ]);

  const handleToggleEnabled = useCallback(() => {
    updateSettings("enabled", !settings.enabled);
  }, [settings.enabled, updateSettings]);

  const handleToggleAutoSync = useCallback(() => {
    updateSettings("auto_sync", !settings.auto_sync);
  }, [settings.auto_sync, updateSettings]);

  const handleValueChange = useCallback(
    (fieldKey: WebDavTextFieldKey, value: string) => {
      updateSettings(fieldKey, value);
    },
    [updateSettings],
  );

  const handlePresetChange = useCallback(
    (nextPresetId: string) => {
      const preset = WEBDAV_PRESETS.find((item) => item.id === nextPresetId);
      setPresetId(nextPresetId);
      if (preset && preset.id !== "custom") {
        updateSettings("base_url", preset.baseUrl);
      }
    },
    [updateSettings],
  );

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

  const remoteLayoutLabel = useMemo(
    () =>
      remoteInfo?.layout
        ? uiText(
            `远端布局：${remoteInfo.layout}`,
            `Remote layout: ${remoteInfo.layout}`,
            `リモート構成: ${remoteInfo.layout}`,
          )
        : uiText("尚未获取远端信息", "Remote info not loaded yet", "リモート情報は未取得です"),
    [remoteInfo?.layout, uiText],
  );

  const passwordPlaceholder = useMemo(
    () =>
      settings.has_password && !passwordTouched && savedAccount
        ? uiText(
            "已保存，留空则保持不变",
            "Saved already. Leave blank to keep it.",
            "保存済みです。空欄なら保持します。",
          )
        : "",
    [passwordTouched, savedAccount, settings.has_password, uiText],
  );

  const presetHint = useMemo(
    () => (activePreset ? uiText(activePreset.hint, activePreset.hint, activePreset.hint) : ""),
    [activePreset, uiText],
  );

  const settingsFields = useMemo<WebDavTextFieldProps[]>(
    () => [
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
    ],
    [
      busy,
      handleValueChange,
      passwordPlaceholder,
      settings.base_url,
      settings.password,
      settings.profile,
      settings.remote_root,
      settings.username,
      uiText,
    ],
  );

  const remoteStatusIcon = remoteInfo?.exists ? (remoteInfo.compatible ? CheckCircle : AlertCircle) : Link2;
  const remoteStatusColor = remoteInfo?.exists
    ? remoteInfo.compatible
      ? "var(--success)"
      : "var(--warning)"
    : "var(--text-secondary)";
  const remoteStatusValue = remoteInfo?.exists
    ? remoteInfo.compatible
      ? remoteInfo.encrypted
        ? uiText("加密备份", "Encrypted backup")
        : uiText("旧备份，未加密", "Legacy, unencrypted backup")
      : uiText(
          "远端备份信息无效或不兼容",
          "Invalid or incompatible remote backup",
          "リモートバックアップ情報が無効、または互換性がありません",
        )
    : remoteInfo
      ? uiText("远端暂无快照", "No remote snapshot yet", "リモートにスナップショットはありません")
      : uiText("尚未读取", "Not loaded", "未取得");
  const remoteStatusDetail = remoteInfo?.updated_at
    ? formatDateTime(remoteInfo.updated_at)
    : remoteInfo?.exists
      ? uiText("备份时间不可用", "Backup timestamp unavailable")
      : remoteInfo
        ? uiText("等待首次上传", "Waiting for the first upload", "最初のアップロード待ち")
        : uiText("刷新以读取远端状态", "Refresh to read remote status");
  const remoteSizeDetail = remoteInfo?.app_version
    ? `CCHub ${remoteInfo.app_version}`
    : uiText("尚未获取版本信息", "Version info unavailable", "バージョン情報は未取得です");
  const remotePathDetail =
    remoteInfo?.protocol_version || remoteInfo?.db_compat_version
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
          "将配置加密备份到你的 WebDAV 存储，在其他设备上恢复。登录密码和备份密码分开保存在系统密钥环中。",
          "Back up encrypted configuration to WebDAV and restore it on another device. Login and backup passwords are stored separately in the OS keyring.",
          "設定を WebDAV にバックアップし、別の端末で復元できます。パスワードはシステムキーチェーンに保存され、対応するサーバーとアカウントにのみ使用されます。",
        )}
      </p>
      {dirty && (
        <p role="status" className="mb-4 text-xs text-[var(--text-secondary)]">
          {uiText(
            "有未保存的修改，请保存后再读取、上传或恢复远端备份。",
            "Save your changes before reading, uploading, or restoring remote backups.",
          )}
        </p>
      )}
      {!savedSettings && !busy && (
        <Button variant="secondary" onClick={() => void loadState()}>
          {uiText("重新读取设置", "Retry loading settings")}
        </Button>
      )}

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
          description={settings.enabled ? uiText("已启用", "Enabled", "有効") : uiText("未启用", "Disabled", "無効")}
          enabled={settings.enabled}
          disabled={busy}
          onToggle={handleToggleEnabled}
        />

        <WebDavToggleCard
          title={uiText("定时自动同步", "Automatic Sync", "自動同期")}
          description={
            settings.auto_sync
              ? uiText("每 15 分钟尝试上传一次", "Attempt upload every 15 minutes", "15 分ごとにアップロードを試行")
              : uiText("仅手动同步", "Manual sync only", "手動同期のみ")
          }
          enabled={settings.auto_sync}
          disabled={busy || (!settings.auto_sync && !passwordAvailable)}
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
          <SimpleSelect
            value={presetId}
            disabled={busy}
            onValueChange={handlePresetChange}
            options={WEBDAV_PRESETS.map((preset) => ({ value: preset.id, label: preset.label }))}
            ariaLabel={uiText("服务预设", "Service Preset", "サービスプリセット")}
          />
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 6 }}>{presetHint}</div>
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
        <BackupEncryptionField
          value={settings.backup_encryption}
          sameLocation={sameBackup}
          disabled={busy}
          text={uiText}
          onChange={(value) => updateSettings("backup_encryption", value)}
        />
      </div>
      <CloudUploadStatus review={remoteInfo?.upload_review} text={uiText} />
      {!passwordAvailable && savedSettings && (
        <p className="mb-3 text-xs text-[var(--text-secondary)]">
          {uiText(
            "请设置并保存备份密码以启用上传和加密备份恢复。",
            "Set and save a backup password to upload or restore encrypted backups.",
          )}
        </p>
      )}

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
        <WebDavActionButton
          label={
            actionState === "saving"
              ? uiText("保存中...", "Saving...", "保存中...")
              : uiText("保存设置", "Save Settings", "設定を保存")
          }
          loading={actionState === "saving"}
          disabled={busy || !savedSettings}
          icon={Save}
          variant="btn-primary"
          onClick={handleSaveClick}
        />
        <WebDavActionButton
          label={
            actionState === "testing"
              ? uiText("测试中...", "Testing...", "テスト中...")
              : uiText("测试连接", "Test Connection", "接続テスト")
          }
          loading={actionState === "testing"}
          disabled={busy || !savedSettings}
          icon={Wifi}
          onClick={handleTestClick}
        />
        <WebDavActionButton
          label={
            actionState === "refreshing"
              ? uiText("刷新中...", "Refreshing...", "更新中...")
              : uiText("刷新远端", "Refresh Remote", "リモートを更新")
          }
          loading={actionState === "refreshing"}
          disabled={remoteActionsDisabled}
          icon={RefreshCw}
          onClick={handleRefreshRemoteClick}
        />
        <WebDavActionButton
          label={
            actionState === "reviewing"
              ? uiText("检查备份...", "Checking backup...", "バックアップを確認中...")
              : actionState === "uploading"
                ? uiText("上传中...", "Uploading...", "アップロード中...")
                : uiText("上传当前快照", "Upload Snapshot", "現在のスナップショットをアップロード")
          }
          loading={actionState === "reviewing" || actionState === "uploading"}
          disabled={remoteActionsDisabled || !passwordAvailable}
          icon={Upload}
          onClick={handleUploadClick}
        />
        <WebDavActionButton
          label={
            actionState === "downloading"
              ? uiText("恢复中...", "Restoring...", "復元中...")
              : uiText("从远端恢复", "Restore From Remote", "リモートから復元")
          }
          loading={actionState === "downloading"}
          disabled={
            remoteActionsDisabled ||
            !remoteInfo?.exists ||
            !remoteInfo.compatible ||
            (!!remoteInfo.encrypted && !passwordAvailable)
          }
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

      {settings.has_password && !passwordTouched && savedAccount && (
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
            "此服务器与账号的密码已保存。留空保持不变；编辑后清空可删除已保存密码。",
            "The password for this server and account is saved. Leave it untouched to keep it; edit and clear it to remove it.",
            "このサーバーとアカウントのパスワードは保存済みです。未編集なら保持し、編集後に空にすると削除します。",
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
