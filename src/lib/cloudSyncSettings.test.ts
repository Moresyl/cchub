import { describe, expect, it } from "vitest";
import {
  cloudSettingsChanged,
  sameS3Account,
  sameWebDavAccount,
  sameS3BackupLocation,
  sameWebDavBackupLocation,
} from "./cloudSyncSettings";

describe("cloud sync account identity", () => {
  it("keeps encryption passwords scoped to the DAV account, root and profile", () => {
    const backup = {
      base_url: "https://dav.test/root",
      username: "alice",
      remote_root: "cchub-sync",
      profile: "default",
    };
    expect(sameWebDavBackupLocation(backup, { ...backup, remote_root: " /cchub-sync/ ", profile: "" })).toBe(true);
    for (const change of [
      { username: "bob" },
      { remote_root: "other" },
      { profile: "other" },
      { base_url: "https://other.test" },
    ])
      expect(sameWebDavBackupLocation(backup, { ...backup, ...change })).toBe(false);
  });

  it("keeps encryption passwords scoped to the S3 account, bucket, root and profile", () => {
    const backup = {
      endpoint: "https://s3.test/storage",
      region: "us-east-1",
      accessKeyId: "alice",
      bucket: "backup",
      remoteRoot: "cchub-sync",
      profile: "default",
    };
    expect(
      sameS3BackupLocation(backup, { ...backup, bucket: " backup ", remoteRoot: "/cchub-sync/", profile: "" }),
    ).toBe(true);
    expect(sameS3BackupLocation(backup, { ...backup, region: "eu-west-1" })).toBe(true);
    for (const change of [
      { accessKeyId: "bob" },
      { bucket: "other" },
      { remoteRoot: "other" },
      { profile: "other" },
      { endpoint: "https://other.test" },
    ])
      expect(sameS3BackupLocation(backup, { ...backup, ...change })).toBe(false);
  });
  it("normalizes DAV URL and username but keeps remote paths and accounts separate", () => {
    const account = { base_url: "https://dav.test/root", username: "alice" };
    expect(sameWebDavAccount(account, { base_url: " HTTPS://DAV.test:443/root/// ", username: " alice " })).toBe(true);
    expect(sameWebDavAccount(account, { ...account, username: "bob" })).toBe(false);
    expect(sameWebDavAccount(account, { ...account, base_url: "https://dav.test/other" })).toBe(false);
  });

  it("uses the effective S3 endpoint and never reuses a secret for another key ID", () => {
    const account = { endpoint: "", region: "us-east-1", accessKeyId: "alice" };
    expect(sameS3Account(account, { ...account, endpoint: "https://s3.us-east-1.amazonaws.com/" })).toBe(true);
    expect(sameS3Account(account, { ...account, region: "eu-west-1" })).toBe(false);
    expect(sameS3Account(account, { ...account, accessKeyId: "bob" })).toBe(false);
    const custom = { ...account, endpoint: "https://s3.test/storage" };
    expect(sameS3Account(custom, { ...custom, region: "eu-west-1" })).toBe(true);
    expect(sameS3Account(custom, { ...custom, endpoint: "https://s3.test/other" })).toBe(false);
  });

  it("checks persisted form fields without treating sync status updates as edits", () => {
    const saved = { endpoint: "https://s3.test", enabled: true, lastError: null as string | null };
    expect(cloudSettingsChanged(saved, null, ["endpoint", "enabled"])).toBe(false);
    expect(cloudSettingsChanged({ ...saved, lastError: "offline" }, saved, ["endpoint", "enabled"])).toBe(false);
    expect(cloudSettingsChanged({ ...saved, enabled: false }, saved, ["endpoint", "enabled"])).toBe(true);
  });
});
