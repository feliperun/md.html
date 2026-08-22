import { test } from "node:test";
import assert from "node:assert/strict";
import { hydrateImages } from "../src/assets.js";
import { appDocument } from "./fake-dom.mjs";

function image(doc, path, attributes = {}) {
  const element = doc.createElement("img");
  element.setAttribute("data-md-asset-path", path);
  element.setAttribute("alt", attributes.alt ?? path);
  if (attributes.title !== undefined) element.setAttribute("title", attributes.title);
  doc.app.appendChild(element);
  return element;
}

function asset(doc, path, mime, payload) {
  const block = doc.createElement("script");
  block.setAttribute("type", "application/octet-stream");
  block.setAttribute("data-path", path);
  block.setAttribute("data-type", mime);
  block.textContent = payload;
  doc.body.appendChild(block);
  return block;
}

function base64(bytes) {
  return Buffer.from(bytes).toString("base64");
}

test("hydrates the 32767-byte boundary immediately and normalizes base64 whitespace", () => {
  const doc = appDocument();
  const small = image(doc, "small.png");
  const payload = base64(new Uint8Array(32767).fill(65));
  asset(doc, "small.png", "image/png", `\n${payload.slice(0, 40)} \n${payload.slice(40)}\t`);

  const result = hydrateImages(doc);

  assert.equal(result.warnings.length, 0);
  assert.equal(small.getAttribute("src"), `data:image/png;base64,${payload}`);
  assert.equal(small.getAttribute("data-md-asset-ready"), "");
  assert.equal(doc.defaultView.intersectionObservers.length, 0);
});

test("preserves hostile image attributes while hydrating by exact path", () => {
  const doc = appDocument();
  const hostile = image(doc, "safe.png", { alt: '<>&"', title: "'\"<>" });
  asset(doc, "safe.png", "image/png", "AQID");

  hydrateImages(doc);

  assert.equal(hostile.getAttribute("alt"), '<>&"');
  assert.equal(hostile.getAttribute("title"), "'\"<>");
  assert.equal(hostile.getAttribute("src"), "data:image/png;base64,AQID");
});

test("defers 32768-byte payloads until intersection and uses the owning window primitives", () => {
  const doc = appDocument();
  const large = image(doc, "large.jpg");
  const payload = base64(new Uint8Array(32768).fill(66));
  asset(doc, "large.jpg", "image/jpeg", payload);
  let atobCalls = 0;
  const originalAtob = doc.defaultView.atob;
  doc.defaultView.atob = (value) => {
    atobCalls += 1;
    return originalAtob(value);
  };

  const result = hydrateImages(doc);
  const observer = doc.defaultView.intersectionObservers[0];

  assert.equal(result.warnings.length, 0);
  assert.equal(large.getAttribute("src"), null);
  assert.equal(large.getAttribute("data-md-asset-ready"), null);
  assert.equal(atobCalls, 0);
  assert.equal(observer.observed.length, 1);

  observer.trigger([{ target: large, isIntersecting: true }]);

  assert.equal(atobCalls, 1);
  assert.equal(large.getAttribute("src"), "blob:document");
  assert.equal(large.getAttribute("data-md-asset-ready"), "");
  assert.deepEqual(observer.unobserved, [large]);
  assert.equal(doc.defaultView.URL.blob.type, "image/jpeg");
});

test("removes a pre-existing src before observing a deferred large image", () => {
  const doc = appDocument();
  const large = image(doc, "large.jpg");
  large.setAttribute("src", "relative-fallback.jpg");
  const payload = base64(new Uint8Array(32768).fill(66));
  asset(doc, "large.jpg", "image/jpeg", payload);
  const primitiveCalls = [];
  let atobCalls = 0;
  let observedSrc;
  let observedPrimitiveCalls;
  const OriginalObserver = doc.defaultView.IntersectionObserver;
  doc.defaultView.IntersectionObserver = class extends OriginalObserver {
    observe(element) {
      observedSrc = element.getAttribute("src");
      observedPrimitiveCalls = [...primitiveCalls];
      super.observe(element);
    }
  };
  const originalAtob = doc.defaultView.atob;
  doc.defaultView.atob = (value) => {
    atobCalls += 1;
    primitiveCalls.push("atob");
    return originalAtob(value);
  };
  const OriginalUint8Array = doc.defaultView.Uint8Array;
  doc.defaultView.Uint8Array = class extends OriginalUint8Array {
    constructor(...args) {
      primitiveCalls.push("Uint8Array");
      super(...args);
    }
  };
  const OriginalBlob = doc.defaultView.Blob;
  doc.defaultView.Blob = class extends OriginalBlob {
    constructor(...args) {
      primitiveCalls.push("Blob");
      super(...args);
    }
  };
  const originalCreateObjectURL = doc.defaultView.URL.createObjectURL;
  doc.defaultView.URL.createObjectURL = (...args) => {
    primitiveCalls.push("createObjectURL");
    return originalCreateObjectURL(...args);
  };

  hydrateImages(doc);

  assert.equal(observedSrc, null);
  assert.deepEqual(observedPrimitiveCalls, []);
  assert.equal(large.getAttribute("src"), null);
  assert.deepEqual(primitiveCalls, []);

  const observer = doc.defaultView.intersectionObservers[0];
  observer.trigger([{ target: large, isIntersecting: true }]);

  assert.equal(large.getAttribute("src"), "blob:document");
  assert.equal(large.getAttribute("data-md-asset-ready"), "");
  assert.equal(atobCalls, 1);
  assert.deepEqual(observer.unobserved, [large]);
  assert.equal(observer.observed.length, 0);
});

test("reports unavailable paths, duplicate blocks, MIME, and malformed base64 in image order", () => {
  const doc = appDocument();
  const missing = image(doc, "missing.png");
  const duplicate = image(doc, "duplicate.png");
  const unsupported = image(doc, "font.woff2");
  const malformed = image(doc, "bad.png");
  asset(doc, "duplicate.png", "image/png", "AQI=");
  asset(doc, "duplicate.png", "image/png", "AwQ=");
  asset(doc, "font.woff2", "font/woff2", "AQI=");
  asset(doc, "bad.png", "image/png", "A===");

  const result = hydrateImages(doc);

  assert.deepEqual(result.warnings, [
    { code: "W-UI-04", path: "missing.png" },
    { code: "W-UI-04", path: "duplicate.png" },
    { code: "W-UI-04", path: "font.woff2" },
    { code: "W-UI-04", path: "bad.png" },
  ]);
  for (const element of [missing, duplicate, unsupported, malformed]) {
    assert.equal(element.getAttribute("src"), null);
    assert.equal(element.getAttribute("data-md-asset-missing"), "");
  }
});

test("missing browser primitives degrade large images without decoding", () => {
  for (const primitive of ["atob", "Uint8Array", "Blob", "IntersectionObserver", "URL"]) {
    const doc = appDocument();
    const large = image(doc, `${primitive}.png`);
    asset(doc, `${primitive}.png`, "image/png", base64(new Uint8Array(32768)));
    if (primitive === "URL") delete doc.defaultView.URL.createObjectURL;
    else delete doc.defaultView[primitive];

    const result = hydrateImages(doc);

    assert.deepEqual(result.warnings, [{ code: "W-UI-04", path: `${primitive}.png` }], primitive);
    assert.equal(large.getAttribute("src"), null, primitive);
    assert.equal(large.getAttribute("data-md-asset-missing"), "", primitive);
    assert.equal(doc.defaultView.intersectionObservers.length, 0, primitive);
  }
});

test("delayed failures update the returned warning array in image order", () => {
  const doc = appDocument();
  const delayed = image(doc, "first.png");
  const immediate = image(doc, "second.png");
  asset(doc, "first.png", "image/png", base64(new Uint8Array(32768)));
  asset(doc, "second.png", "image/png", "A===");
  doc.defaultView.atob = () => { throw new Error("synthetic decode failure"); };

  const result = hydrateImages(doc);
  assert.deepEqual(result.warnings, [{ code: "W-UI-04", path: "second.png" }]);
  doc.defaultView.intersectionObservers[0].trigger([{ target: delayed, isIntersecting: true }]);
  assert.deepEqual(result.warnings, [
    { code: "W-UI-04", path: "first.png" },
    { code: "W-UI-04", path: "second.png" },
  ]);
});

test("terminal images are skipped by repeated hydration", () => {
  const doc = appDocument();
  const ready = image(doc, "ready.png");
  const missing = image(doc, "missing.png");
  asset(doc, "ready.png", "image/png", "AQI=");
  const first = hydrateImages(doc);
  const observerCount = doc.defaultView.intersectionObservers.length;
  const second = hydrateImages(doc);

  assert.equal(first.images.length, 2);
  assert.equal(second.images.length, 2);
  assert.equal(doc.defaultView.intersectionObservers.length, observerCount);
  assert.deepEqual(second.warnings, []);
  assert.equal(ready.getAttribute("data-md-asset-ready"), "");
  assert.equal(missing.getAttribute("data-md-asset-missing"), "");
});
