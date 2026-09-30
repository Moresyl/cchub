import type { ConfirmOptions } from "../components/AppDialogProvider";

export interface CloudUploadReview {
  revision: string;
  requiresConfirmation: boolean;
  conditionalSupported: boolean;
}

export async function confirmCloudUpload(
  review: CloudUploadReview | null | undefined,
  exists: boolean,
  updatedAt: string | null,
  confirm: (options: ConfirmOptions) => Promise<boolean>,
  text: (zh: string, en: string) => string,
): Promise<string | null> {
  if (!review || !/^[a-f0-9]{64}$/i.test(review.revision)) {
    throw new Error(
      text(
        "未取得有效的远端版本，请刷新远端后重试。",
        "No valid remote revision was received. Refresh remote status and try again.",
      ),
    );
  }
  if (!review.conditionalSupported) {
    throw new Error(
      text(
        "存储未提供安全替换所需的版本标识，上传已停止；仍可下载备份。",
        "Storage did not provide a revision for safe replacement. Upload stopped; backups can still be downloaded.",
      ),
    );
  }
  if (review.requiresConfirmation) {
    const timestamp =
      updatedAt && !Number.isNaN(Date.parse(updatedAt))
        ? new Date(updatedAt).toLocaleString()
        : text("时间未知", "unknown time");
    const confirmed = await confirm({
      title: exists
        ? text("替换远端备份", "Replace remote backup")
        : text("重新创建远端备份", "Recreate remote backup"),
      message: exists
        ? text(
            `远端备份（${timestamp}）尚未与本机同步。可以先恢复该备份；继续替换会将本地配置设为当前远端备份，旧快照仍保留。`,
            `The remote backup (${timestamp}) has not been synced to this device. You can restore it first. Replacing it makes your local configuration the current remote backup; the previous snapshot is retained.`,
          )
        : text(
            "远端备份已删除。继续会用本地配置重新创建远端备份。",
            "The remote backup was deleted. Continuing recreates it using your local configuration.",
          ),
      confirmText: exists ? text("替换备份", "Replace backup") : text("重新创建", "Recreate"),
      cancelText: text("取消", "Cancel"),
      tone: "warning",
    });
    if (!confirmed) return null;
  }
  return review.revision;
}
