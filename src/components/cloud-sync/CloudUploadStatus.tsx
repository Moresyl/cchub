import type { CloudUploadReview } from "../../lib/cloudUploadReview";

export function CloudUploadStatus({
  review,
  text,
}: {
  review?: CloudUploadReview | null;
  text: (zh: string, en: string) => string;
}) {
  if (!review || (review.conditionalSupported && !review.requiresConfirmation)) return null;
  return (
    <p
      role="status"
      className="mb-3 rounded-lg border border-[var(--border-default)] bg-[var(--control-background)] px-3 py-2 text-xs leading-normal text-[var(--text-secondary)]"
    >
      {review.conditionalSupported
        ? text(
            "远端版本尚未与本机同步。可以先恢复；上传替换前会提示确认。自动上传不会覆盖这个版本。",
            "This remote revision has not been synced to this device. Restore it first or confirm its replacement when uploading. Automatic uploads cannot replace this revision.",
          )
        : text(
            "存储未提供安全替换所需的版本标识。可以下载备份，上传已停止，请使用支持条件写入的存储。",
            "Storage did not provide a revision for safe replacement. You can download backups, but uploading is stopped. Use storage that supports conditional writes.",
          )}
    </p>
  );
}
