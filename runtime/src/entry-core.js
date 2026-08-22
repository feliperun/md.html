import { mountCore } from "./core.js";

const stateKey = Symbol.for("mdhtml.runtime.1");

if (typeof document !== "undefined") {
  globalThis[stateKey] = mountCore(document);
}
