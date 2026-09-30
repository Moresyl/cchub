import { describe, expect, it } from "vitest";
import { cloudSettingsChanged, sameS3Account, sameWebDavAccount } from "./cloudSyncSettings";

describe("cloud sync account identity", () => {
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
