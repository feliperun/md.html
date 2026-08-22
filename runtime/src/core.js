import { FrontMatterError, parseFrontMatter } from "./frontmatter.js";
import { decodeCanonicalSource, projectMarkdown } from "./canonical.js";
import { renderMarkdown } from "./render.js";
import { mountStyles } from "./styles.js";
import { hydrateImages } from "./assets.js";

const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);

function formatIsValid(doc) {
  const root = doc?.documentElement;
  if (!root || root.getAttribute("data-mdhtml") !== "1.0") return false;
  const sources = doc.querySelectorAll('script[type="text/markdown"]');
  const apps = doc.querySelectorAll("#mdhtml-app");
  return sources.length === 1 && sources[0].id === "mdhtml-source" && apps.length === 1;
}

function parseCode(error) {
  return error instanceof FrontMatterError && error.code === "E-PARSE-01"
    ? error.code
    : "E-PARSE-01";
}

export function mountDocument(doc) {
  if (!formatIsValid(doc)) return { ok: false, code: "E-FMT-01" };

  const sourceElement = doc.querySelectorAll('script[type="text/markdown"]')[0];
  const app = doc.querySelectorAll("#mdhtml-app")[0];
  const source = sourceElement.textContent;
  let parsed;
  let rendered;
  try {
    parsed = parseFrontMatter(decodeCanonicalSource(source));
    rendered = own(parsed.frontMatter, "sections")
      ? renderMarkdown(parsed.body, { sections: parsed.frontMatter.sections })
      : renderMarkdown(parsed.body);
  } catch (error) {
    const code = parseCode(error);
    app.textContent = `Unable to render document (${code}).`;
    return { ok: false, code };
  }

  app.innerHTML = rendered.html;
  doc.documentElement.setAttribute("data-mdhtml-runtime", "1.0");
  doc.documentElement.setAttribute("data-mdhtml-ready", "");
  const result = {
    ok: true,
    frontMatter: parsed.frontMatter,
    html: rendered.html,
    headings: rendered.headings,
    warnings: rendered.warnings,
  };
  if (rendered.bindings !== undefined) result.bindings = rendered.bindings;
  if (rendered.errors !== undefined) result.errors = rendered.errors;
  return result;
}

export function mountCore(doc) {
  const sourceElement = doc?.querySelectorAll?.('script[type="text/markdown"]')?.[0];
  const storedSource = sourceElement?.textContent ?? null;
  const result = mountDocument(doc);
  const images = [];
  if (result.ok) {
    const assets = hydrateImages(doc);
    result.warnings.push(...assets.warnings);
    images.push(...assets.images);
    mountStyles(doc, result.frontMatter.theme);
  }
  return Object.freeze({ result, storedSource, images, projectMarkdown });
}
