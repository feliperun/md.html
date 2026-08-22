import { mountChrome } from "./chrome.js";

const state = globalThis[Symbol.for("mdhtml.runtime.1")];
if (typeof document !== "undefined" && state?.result?.ok === true) {
  mountChrome(document, state.storedSource, state.projectMarkdown);
}
