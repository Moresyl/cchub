import { useId } from "react";
import { Input } from "../ui/input";
import type { BackupEncryptionSettings } from "../../lib/backupEncryption";

export function BackupEncryptionField({
  value,
  sameLocation,
  disabled,
  text,
  onChange,
}: {
  value: BackupEncryptionSettings;
  sameLocation: boolean;
  disabled: boolean;
  text: (zh: string, en: string) => string;
  onChange: (value: BackupEncryptionSettings) => void;
}) {
  const id = useId();
  return (
    <div className="min-w-0 text-xs text-[var(--text-secondary)]">
      <label htmlFor={id} className="mb-1.5 block">
        {text("备份密码", "Backup password")}
      </label>
      <Input
        id={id}
        type="password"
        autoComplete="new-password"
        value={value.passphrase}
        placeholder={
          value.hasPassphrase && !value.passphraseTouched && sameLocation
            ? text("已保存，留空保持不变", "Saved; leave blank to keep")
            : text("至少 12 个字符", "At least 12 characters")
        }
        disabled={disabled}
        aria-describedby={`${id}-help`}
        onChange={(event) => onChange({ ...value, passphrase: event.target.value, passphraseTouched: true })}
      />
      <p id={`${id}-help`} className="mt-1.5 text-[11px] leading-normal text-[var(--text-muted)]">
        {text(
          "用于加密备份，与登录密码分开。其他设备恢复时需要同一密码，请自行保管；编辑后清空可删除已保存密码。",
          "Encrypts backups separately from your login password. Keep it safe: other devices need the same password to restore. Edit and clear to remove the saved password.",
        )}
      </p>
    </div>
  );
}
