import { describe, expect, it, vi } from "vitest";
import { confirmCloudUpload, type CloudUploadReview } from "./cloudUploadReview";

const text = (zh: string) => zh;
const known: CloudUploadReview = { revision: "a".repeat(64), requiresConfirmation: false, conditionalSupported: true };

describe("manual cloud upload review", () => {
  it("passes the exact observed revision without prompting for an accepted version", async () => {
    const confirm = vi.fn();
    expect(await confirmCloudUpload(known, true, null, confirm, text)).toBe(known.revision);
    expect(confirm).not.toHaveBeenCalled();
  });

  it("prompts for a new remote backup and cancellation never authorizes upload", async () => {
    const confirm = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    const review = { ...known, requiresConfirmation: true };
    expect(await confirmCloudUpload(review, true, "2026-09-30T12:00:00Z", confirm, text)).toBeNull();
    expect(confirm).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "替换远端备份",
        confirmText: "替换备份",
        tone: "warning",
        message: expect.stringContaining("旧快照仍保留"),
      }),
    );
    expect(await confirmCloudUpload(review, true, "invalid", confirm, text)).toBe(known.revision);
    expect(confirm.mock.calls[1][0].message).toContain("时间未知");
  });

  it("requires deliberate recreation after remote deletion", async () => {
    const confirm = vi.fn().mockResolvedValue(true);
    await confirmCloudUpload({ ...known, requiresConfirmation: true }, false, null, confirm, text);
    expect(confirm).toHaveBeenCalledWith(
      expect.objectContaining({ title: "重新创建远端备份", message: expect.stringContaining("已删除") }),
    );
  });

  it("blocks unsupported conditions and invalid metadata without offering an unsafe fallback", async () => {
    const confirm = vi.fn();
    for (const review of [undefined, null, { ...known, revision: "bad" }, { ...known, conditionalSupported: false }]) {
      await expect(confirmCloudUpload(review, true, null, confirm, text)).rejects.toThrow();
    }
    expect(confirm).not.toHaveBeenCalled();
  });
});
