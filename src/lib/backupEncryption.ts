export interface BackupEncryptionSettings {
  passphrase: string;
  hasPassphrase: boolean;
  passphraseTouched: boolean;
}

export const EMPTY_BACKUP_ENCRYPTION: BackupEncryptionSettings = {
  passphrase: "",
  hasPassphrase: false,
  passphraseTouched: false,
};

export function maskBackupEncryption(value?: Partial<BackupEncryptionSettings>): BackupEncryptionSettings {
  return { ...EMPTY_BACKUP_ENCRYPTION, hasPassphrase: value?.hasPassphrase ?? false };
}

export function backupPasswordAvailable(value: BackupEncryptionSettings, sameLocation: boolean) {
  if (value.passphraseTouched || value.passphrase !== "") {
    return (
      value.passphrase.trim() !== "" &&
      [...value.passphrase].length >= 12 &&
      new TextEncoder().encode(value.passphrase).length <= 1024
    );
  }
  return sameLocation && value.hasPassphrase;
}
