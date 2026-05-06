import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");
const srcDir = path.join(rootDir, "src");
const supportedExtensions = new Set([".ts", ".tsx", ".js", ".jsx"]);

function walk(dir) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return walk(fullPath);
    }
    if (!entry.isFile() || !supportedExtensions.has(path.extname(entry.name))) {
      return [];
    }
    return [fullPath];
  });
}

function countInlineStyles(source) {
  const matches = source.match(/\bstyle\s*=\s*\{\{/g);
  return matches?.length ?? 0;
}

const results = walk(srcDir)
  .map((filePath) => ({
    filePath: path.relative(rootDir, filePath).replaceAll(path.sep, "/"),
    count: countInlineStyles(fs.readFileSync(filePath, "utf8")),
  }))
  .filter((result) => result.count > 0)
  .sort((a, b) => b.count - a.count || a.filePath.localeCompare(b.filePath));

const total = results.reduce((sum, result) => sum + result.count, 0);

console.log("Inline style count by file:");
for (const result of results) {
  console.log(`${String(result.count).padStart(4, " ")}  ${result.filePath}`);
}
console.log(`\nTotal: ${total}`);
