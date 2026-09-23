import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const configPath = resolve(projectRoot, "src-tauri/tauri.conf.json");
const packagePath = resolve(projectRoot, "package.json");

const config = JSON.parse(readFileSync(configPath, "utf8"));
const packageJson = JSON.parse(readFileSync(packagePath, "utf8"));
const windows = config.bundle?.windows;
const nsis = windows?.nsis;
const wix = windows?.wix;

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function verifyBitmap(relativePath, expectedWidth, expectedHeight) {
  const absolutePath = resolve(projectRoot, "src-tauri", relativePath);
  assert(existsSync(absolutePath), `Missing installer bitmap: ${relativePath}`);

  const bitmap = readFileSync(absolutePath);
  assert(bitmap.toString("ascii", 0, 2) === "BM", `${relativePath} is not a BMP file`);
  assert(bitmap.readInt32LE(18) === expectedWidth, `${relativePath} must be ${expectedWidth}px wide`);
  assert(Math.abs(bitmap.readInt32LE(22)) === expectedHeight, `${relativePath} must be ${expectedHeight}px high`);
  assert(bitmap.readUInt16LE(28) === 24, `${relativePath} must use 24-bit RGB pixels`);
}

assert(
  /^\^?2\.11\.5$/.test(packageJson.devDependencies?.["@tauri-apps/cli"] ?? ""),
  "Tauri CLI 2.11.5 is required for uninstaller branding",
);
assert(windows && nsis && wix, "Both NSIS and WiX installer configuration must be present");
assert(nsis.installerIcon === "icons/icon.ico", "NSIS installer must use the CCHub icon");
assert(nsis.uninstallerIcon === "icons/icon.ico", "NSIS uninstaller must use the CCHub icon");
assert(nsis.installMode === "currentUser", "NSIS must keep the current-user install mode");
assert(
  JSON.stringify(nsis.languages) === JSON.stringify(["SimpChinese", "English"]),
  "NSIS language order must prefer Simplified Chinese with English fallback",
);
assert(wix.upgradeCode === "0f196973-5299-5133-b433-da6f8f0cbd91", "WiX upgrade code must remain stable");

for (const languagePath of Object.values(nsis.customLanguageFiles ?? {})) {
  assert(existsSync(resolve(projectRoot, "src-tauri", languagePath)), `Missing NSIS language file: ${languagePath}`);
}

verifyBitmap(nsis.sidebarImage, 164, 314);
verifyBitmap(nsis.headerImage, 150, 57);
verifyBitmap(nsis.uninstallerHeaderImage, 150, 57);
verifyBitmap(wix.bannerPath, 493, 58);
verifyBitmap(wix.dialogImagePath, 493, 312);

console.log("Windows installer branding assets are valid.");
