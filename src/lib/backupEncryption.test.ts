import { describe, expect, it } from "vitest";
import { backupPasswordAvailable, EMPTY_BACKUP_ENCRYPTION, maskBackupEncryption } from "./backupEncryption";

describe("backup encryption form", () => {
  it("clears plaintext and edit flags from server responses", () => {
    expect(
      maskBackupEncryption({ passphrase: "server-should-not-return", hasPassphrase: true, passphraseTouched: true }),
    ).toEqual({ ...EMPTY_BACKUP_ENCRYPTION, hasPassphrase: true });
    expect(maskBackupEncryption()).toEqual(EMPTY_BACKUP_ENCRYPTION);
  });

  it("uses saved passwords only for the same backup location and respects explicit removal", () => {
    const saved = { ...EMPTY_BACKUP_ENCRYPTION, hasPassphrase: true };
    expect(backupPasswordAvailable(saved, true)).toBe(true);
    expect(backupPasswordAvailable(saved, false)).toBe(false);
    expect(backupPasswordAvailable({ ...saved, passphraseTouched: true }, true)).toBe(false);
  });

  it("matches native password validation for Unicode, blank and byte limits", () => {
    const ready = (passphrase: string) =>
      backupPasswordAvailable({ ...EMPTY_BACKUP_ENCRYPTION, passphraseTouched: true, passphrase }, false);
    expect(ready("abcdefghijkl")).toBe(true);
    expect(ready("abcdefghijk")).toBe(false);
    expect(ready(" ".repeat(12))).toBe(false);
    expect(ready("😀".repeat(12))).toBe(true);
    expect(ready("😀".repeat(256))).toBe(true);
    expect(ready("😀".repeat(257))).toBe(false);
  });
});
