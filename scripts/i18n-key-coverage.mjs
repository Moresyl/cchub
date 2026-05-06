import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");
const i18nPath = path.join(rootDir, "src", "lib", "i18n.ts");

const sourceText = fs.readFileSync(i18nPath, "utf8");
const sourceFile = ts.createSourceFile(i18nPath, sourceText, ts.ScriptTarget.Latest, true);
const localeNames = ["zh", "en", "ja"];
const strict = process.argv.includes("--strict");

function findLocaleObject(name) {
  for (const statement of sourceFile.statements) {
    if (!ts.isVariableStatement(statement)) continue;
    for (const declaration of statement.declarationList.declarations) {
      if (declaration.name.getText(sourceFile) !== name) continue;
      if (declaration.initializer && ts.isObjectLiteralExpression(declaration.initializer)) {
        return declaration.initializer;
      }
    }
  }
  throw new Error(`Could not find locale object: ${name}`);
}

function findObjectPath(pathParts) {
  const [rootName, ...properties] = pathParts;
  let current = findLocaleObject(rootName);
  for (const propertyName of properties) {
    const property = current.properties.find((item) => (
      ts.isPropertyAssignment(item)
      && propertyNameText(item.name) === propertyName
      && ts.isObjectLiteralExpression(item.initializer)
    ));
    if (!property || !ts.isPropertyAssignment(property) || !ts.isObjectLiteralExpression(property.initializer)) {
      return null;
    }
    current = property.initializer;
  }
  return current;
}

function expressionPath(expression) {
  if (ts.isIdentifier(expression)) {
    return [expression.text];
  }
  if (ts.isPropertyAccessExpression(expression)) {
    const base = expressionPath(expression.expression);
    return base ? [...base, expression.name.text] : null;
  }
  return null;
}

function propertyNameText(name) {
  if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
    return name.text;
  }
  return name.getText(sourceFile);
}

function collectLeafKeys(node, prefix = "") {
  const keys = [];
  for (const property of node.properties) {
    if (ts.isSpreadAssignment(property)) {
      const spreadPath = expressionPath(property.expression);
      const spreadObject = spreadPath ? findObjectPath(spreadPath) : null;
      if (spreadObject) {
        keys.push(...collectLeafKeys(spreadObject, prefix));
      }
      continue;
    }
    if (!ts.isPropertyAssignment(property)) continue;
    const key = propertyNameText(property.name);
    const pathKey = prefix ? `${prefix}.${key}` : key;
    if (ts.isObjectLiteralExpression(property.initializer)) {
      keys.push(...collectLeafKeys(property.initializer, pathKey));
    } else {
      keys.push(pathKey);
    }
  }
  return keys;
}

const localeKeys = Object.fromEntries(
  localeNames.map((name) => [name, new Set(collectLeafKeys(findLocaleObject(name)))]),
);
const baseline = localeKeys.zh;
let failed = false;

for (const locale of localeNames.filter((name) => name !== "zh")) {
  const current = localeKeys[locale];
  const missing = [...baseline].filter((key) => !current.has(key));
  const extra = [...current].filter((key) => !baseline.has(key));

  if (missing.length || extra.length) {
    failed = true;
    console.error(`Locale ${locale} does not match zh keys.`);
    if (missing.length) {
      console.error(`  Missing (${missing.length}): ${missing.join(", ")}`);
    }
    if (extra.length) {
      console.error(`  Extra (${extra.length}): ${extra.join(", ")}`);
    }
  }
}

if (failed) {
  if (strict) {
    process.exit(1);
  }
  console.warn("i18n key coverage has gaps. Run with --strict to fail on gaps.");
} else {
  console.log(`i18n key coverage OK: ${baseline.size} keys across ${localeNames.join(", ")}`);
}
