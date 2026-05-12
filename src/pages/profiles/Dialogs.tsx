/* eslint-disable @typescript-eslint/no-explicit-any */
import ConfirmDialog from "../../components/ConfirmDialog";
import type { ConfigProfile, ProviderConfigFragment } from "./helpers";

type LocaleText = (zh: string, en: string, ja?: string) => string;

interface ProfilesConfirmDialogsProps {
  locale: string;
  localeText: LocaleText;
  confirmAction: { type: string; profile: ConfigProfile } | null;
  setConfirmAction: React.Dispatch<React.SetStateAction<{ type: string; profile: ConfigProfile } | null>>;
  doDelete: (profile: ConfigProfile) => Promise<void> | void;
  sharedGroupCounts: Record<string, number>;
  confirmFragmentDelete: ProviderConfigFragment | null;
  setConfirmFragmentDelete: (value: ProviderConfigFragment | null) => void;
  doDeleteFragment: (fragment: ProviderConfigFragment) => Promise<void> | void;
  streamCheckConfirmProfile: ConfigProfile | null;
  setStreamCheckConfirmProfile: (value: ConfigProfile | null) => void;
  runStreamCheck: (profile: ConfigProfile) => Promise<void> | void;
}

export default function ProfilesConfirmDialogs({
  locale,
  localeText,
  confirmAction,
  setConfirmAction,
  doDelete,
  sharedGroupCounts,
  confirmFragmentDelete,
  setConfirmFragmentDelete,
  doDeleteFragment,
  streamCheckConfirmProfile,
  setStreamCheckConfirmProfile,
  runStreamCheck,
}: ProfilesConfirmDialogsProps) {
  return (
    <>
      <ConfirmDialog
        isOpen={!!confirmAction}
        title={
          confirmAction?.profile.source_type === "shared"
            ? localeText("删除共享配置", "Delete Shared Provider", "共有 Provider を削除")
            : locale === "zh"
              ? "删除配置"
              : "Delete Configuration"
        }
        message={
          confirmAction?.profile.source_type === "shared" && confirmAction.profile.source_key
            ? localeText(
                `确定删除共享配置「${confirmAction.profile.name}」？这会同时删除 ${sharedGroupCounts[confirmAction.profile.source_key] || 1} 个 App 上的联动配置。`,
                `Delete shared provider "${confirmAction.profile.name}"? This also removes the linked profiles across ${sharedGroupCounts[confirmAction.profile.source_key] || 1} apps.`,
                `共有 Provider「${confirmAction.profile.name}」を削除しますか？ ${sharedGroupCounts[confirmAction.profile.source_key] || 1} 個の App にある連動プロファイルも同時に削除されます。`,
              )
            : locale === "zh"
              ? `确定删除配置「${confirmAction?.profile.name}」？此操作不可撤销。`
              : `Delete "${confirmAction?.profile.name}"? This cannot be undone.`
        }
        confirmText={localeText("删除", "Delete", "削除")}
        variant="destructive"
        onConfirm={() => {
          if (confirmAction) void doDelete(confirmAction.profile);
          setConfirmAction(null);
        }}
        onCancel={() => setConfirmAction(null)}
      />
      <ConfirmDialog
        isOpen={!!confirmFragmentDelete}
        title={localeText("删除配置片段", "Delete Provider Fragment", "Provider フラグメントを削除")}
        message={
          confirmFragmentDelete
            ? localeText(
                `确定删除配置片段「${confirmFragmentDelete.name}」？删除后将无法继续复用这组字段。`,
                `Delete provider fragment "${confirmFragmentDelete.name}"? You will no longer be able to reuse this field set.`,
                `Provider フラグメント「${confirmFragmentDelete.name}」を削除しますか？ このフィールドセットは再利用できなくなります。`,
              )
            : ""
        }
        confirmText={localeText("删除", "Delete", "削除")}
        variant="destructive"
        onConfirm={() => {
          const fragment = confirmFragmentDelete;
          setConfirmFragmentDelete(null);
          if (!fragment) return;
          void doDeleteFragment(fragment);
        }}
        onCancel={() => setConfirmFragmentDelete(null)}
      />
      <ConfirmDialog
        isOpen={!!streamCheckConfirmProfile}
        title={localeText("流式健康检查", "Stream Health Check", "ストリームヘルスチェック")}
        message={localeText(
          "将向 Provider 发送一条最小化的流式请求，用于验证端点是否能成功返回首个流式分片。\n\n首次确认后，后续将直接执行。",
          "CCHub will send a minimal streaming request to verify that this provider endpoint can return the first stream chunk successfully.\n\nAfter you confirm once, future checks will run immediately.",
          "Provider に最小限のストリーミングリクエストを送り、最初のストリームチャンクを正しく返せるか確認します。\n\n一度確認すると、以後はすぐに実行されます。",
        )}
        confirmText={localeText("继续检查", "Run Check", "チェックを実行")}
        cancelText={localeText("取消", "Cancel", "キャンセル")}
        variant="info"
        onConfirm={() => {
          const profile = streamCheckConfirmProfile;
          setStreamCheckConfirmProfile(null);
          if (!profile) return;
          localStorage.setItem("cchub-stream-check-confirmed", "1");
          void runStreamCheck(profile);
        }}
        onCancel={() => setStreamCheckConfirmProfile(null)}
      />
    </>
  );
}
