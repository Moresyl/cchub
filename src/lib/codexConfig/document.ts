import type { AST } from "toml-eslint-parser";
import { parseTomlSource } from "../tomlSyntax";

type Path = (string | number)[];
type Entry = AST.TOMLContentNode | AST.TOMLTable;

function keyPath(key: AST.TOMLKey): string[] {
  return key.keys.map((part) => (part.type === "TOMLBare" ? part.name : part.value));
}

function prefix(path: Path, ancestor: Path) {
  return ancestor.every((part, index) => path[index] === part);
}

function renderedKey(path: Path) {
  return path
    .map((part) => (/^[A-Za-z0-9_-]+$/.test(String(part)) ? String(part) : JSON.stringify(String(part))))
    .join(".");
}

/** Index ranges, rather than converting unknown values through JavaScript objects. */
export class CodexTomlDocument {
  readonly ast: AST.TOMLProgram;
  private readonly entries = new Map<string, { path: Path; node: Entry }>();
  readonly newline: string;

  constructor(readonly content: string) {
    this.newline = content.includes("\r\n") ? "\r\n" : "\n";
    // Preserve source offsets for files carrying a UTF-8 BOM.
    this.ast = parseTomlSource(content);
    const add = (path: Path, node: Entry) => {
      this.entries.set(JSON.stringify(path), { path, node });
      if (node.type === "TOMLInlineTable") {
        for (const entry of node.body) add([...path, ...keyPath(entry.key)], entry.value);
      } else if (node.type === "TOMLArray") {
        node.elements.forEach((entry, index) => add([...path, index], entry));
      }
    };
    for (const entry of this.ast.body[0].body) {
      if (entry.type === "TOMLTable") {
        add(entry.resolvedKey, entry);
        for (const field of entry.body) add([...entry.resolvedKey, ...keyPath(field.key)], field.value);
      } else {
        add(keyPath(entry.key), entry.value);
      }
    }
  }

  get(path: string[]) {
    return this.entries.get(JSON.stringify(path))?.node;
  }

  has(path: string[]) {
    return [...this.entries.values()].some((entry) => prefix(entry.path, path));
  }

  isTable(path: string[]) {
    const node = this.get(path);
    if (node) return node.type === "TOMLInlineTable" || (node.type === "TOMLTable" && node.kind === "standard");
    return [...this.entries.values()].some(
      (entry) => prefix(entry.path, path) && typeof entry.path[path.length] === "string",
    );
  }

  children(path: string[]) {
    const children = new Set<string>();
    for (const entry of this.entries.values()) {
      const child = entry.path[path.length];
      if (prefix(entry.path, path) && typeof child === "string") children.add(child);
    }
    return [...children];
  }

  private splice(start: number, end: number, replacement: string) {
    return this.content.slice(0, start) + replacement + this.content.slice(end);
  }

  set(path: string[], renderedValue: string | null): string {
    const existing = this.get(path);
    if (existing) {
      if (existing.type === "TOMLTable") throw new Error("A table cannot be replaced with a scalar field");
      if (renderedValue !== null) return this.splice(...existing.range, renderedValue);
      const assignment = existing.parent;
      if (assignment.type !== "TOMLKeyValue") throw new Error("Expected a field assignment");
      if (assignment.parent.type === "TOMLInlineTable") {
        const siblings = assignment.parent.body;
        const index = siblings.indexOf(assignment);
        const next = siblings[index + 1];
        const previous = siblings[index - 1];
        if (next) return this.splice(assignment.range[0], next.range[0], "");
        if (previous) return this.splice(previous.range[1], assignment.range[1], "");
      }
      // Leave trailing comments and line endings intact, including multiline values.
      return this.splice(assignment.range[0], assignment.range[1], "");
    }
    if (renderedValue === null) return this.content;
    const ancestors = [...this.entries.values()]
      .filter((entry) => entry.path.length < path.length && prefix(path, entry.path))
      .sort((a, b) => b.path.length - a.path.length);
    for (const { path: ancestor, node } of ancestors) {
      const assignment = `${renderedKey(path.slice(ancestor.length))} = ${renderedValue}`;
      if (node.type === "TOMLInlineTable") {
        const at = node.range[1] - 1;
        return this.splice(at, at, `${node.body.length ? ", " : " "}${assignment} `);
      }
      if (node.type === "TOMLTable" && node.kind === "standard") {
        // Insert immediately after the header, keeping existing body comments in place.
        const end = this.content.indexOf("\n", node.key.range[1]);
        const at = end < 0 ? this.content.length : end + 1;
        return this.splice(at, at, `${end < 0 ? this.newline : ""}${assignment}${this.newline}`);
      }
      throw new Error("The parent of this field must be a TOML table");
    }
    const firstTable = this.ast.body[0].body.find((entry) => entry.type === "TOMLTable");
    const start = firstTable ? this.content.lastIndexOf("\n", firstTable.range[0] - 1) + 1 : this.content.length;
    const at = start === 0 && this.content.startsWith("\uFEFF") ? 1 : start;
    const lead =
      at > 0 && !this.content.slice(0, at).endsWith("\n") && this.content[at - 1] !== "\uFEFF" ? this.newline : "";
    return this.splice(at, at, `${lead}${renderedKey(path)} = ${renderedValue}${this.newline}`);
  }
}
