import { orderedMappingEntries, parseFrontMatter } from "./frontmatter.js";

const encodedTerminator = ["<", "\\", "/", "s", "c", "r", "i", "p", "t"].join("");
const decodedTerminator = ["<", "/", "s", "c", "r", "i", "p", "t"].join("");
const encodedTerminatorPattern = new RegExp(encodedTerminator.replace("\\", "\\\\"), "giu");
const PRESENTATION_KEYS = new Set(["theme", "tokens", "fonts", "sections", "figures", "toc"]);

export function decodeCanonicalSource(source) {
  return source.replace(encodedTerminatorPattern, decodedTerminator);
}

function serializeValue(value) {
  if (value === null) return "null";
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new TypeError("front matter contains a non-finite number");
    return String(value);
  }
  if (Array.isArray(value)) return `[${value.map(serializeValue).join(", ")}]`;
  if (value !== null && typeof value === "object") {
    return `{${orderedMappingEntries(value).map(([key, entry]) => `${JSON.stringify(String(key))}: ${serializeValue(entry)}`).join(", ")}}`;
  }
  throw new TypeError("front matter contains an unsupported value");
}

function serializeSmartFrontMatter(frontMatter) {
  const entries = orderedMappingEntries(frontMatter).filter(([key]) => !PRESENTATION_KEYS.has(key));
  if (entries.length === 0) return "";
  return `---\n${entries.map(([key, value]) => `${JSON.stringify(String(key))}: ${serializeValue(value)}`).join("\n")}\n---\n`;
}

export function projectMarkdown(storedSource, mode = "smart") {
  if (mode === "full") return storedSource;
  if (mode !== "body" && mode !== "smart") throw new TypeError(`unsupported Markdown projection mode: ${mode}`);

  const decodedSource = decodeCanonicalSource(storedSource);
  const parsed = parseFrontMatter(decodedSource);
  if (mode === "body") return parsed.body;
  if (parsed.raw === "") return decodedSource;

  const frontMatter = serializeSmartFrontMatter(parsed.frontMatter);
  return frontMatter === "" ? parsed.body : frontMatter + parsed.body;
}
