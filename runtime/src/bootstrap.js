import { mountCore } from "./core.js";
import { projectMarkdown } from "./canonical.js";
import { mountChrome } from "./chrome.js";
import { mountToc } from "./navigation.js";
import { mountLightbox } from "./lightbox.js";

export { mountDocument } from "./core.js";

export function boot(doc) {
  if (doc === undefined || doc === null) return undefined;
  const evidence = mountCore(doc);
  if (evidence.result.ok) {
    mountChrome(doc, evidence.storedSource, projectMarkdown);
    mountToc(doc, evidence.result.headings, evidence.result.frontMatter.toc);
    mountLightbox(doc, evidence.images);
  }
  return evidence.result;
}

if (typeof document !== "undefined") boot(document);
