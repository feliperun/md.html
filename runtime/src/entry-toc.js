import { mountToc } from "./navigation.js";

const state = globalThis[Symbol.for("mdhtml.runtime.1")];
if (typeof document !== "undefined" && state?.result?.ok === true) {
  mountToc(document, state.result.headings, state.result.frontMatter.toc);
}
