import { mountLightbox } from "./lightbox.js";

const state = globalThis[Symbol.for("mdhtml.runtime.1")];
if (typeof document !== "undefined" && state?.result?.ok === true) {
  mountLightbox(document, state.images);
}
