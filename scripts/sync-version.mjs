import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

const packageJsonPath = path.join(rootDir, "package.json");
const tauriConfPath = path.join(rootDir, "src-tauri", "tauri.conf.json");
const cargoTomlPath = path.join(rootDir, "src-tauri", "Cargo.toml");

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
const version = packageJson.version;

if (!version || typeof version !== "string") {
  throw new Error("package.json version is missing or invalid");
}

const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, "utf8"));
if (tauriConf.version !== version) {
  tauriConf.version = version;
  fs.writeFileSync(tauriConfPath, `${JSON.stringify(tauriConf, null, 2)}\n`);
}

const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
const versionPattern = /^version = ".*"$/m;

if (!versionPattern.test(cargoToml)) {
  throw new Error("Failed to locate package version in src-tauri/Cargo.toml");
}

const nextCargoToml = cargoToml.replace(
  /^version = ".*"$/m,
  `version = "${version}"`,
);

if (nextCargoToml !== cargoToml) {
  fs.writeFileSync(cargoTomlPath, nextCargoToml);
}

console.log(`Synchronized app version to ${version}`);
