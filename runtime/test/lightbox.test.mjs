import { test } from "node:test";
import assert from "node:assert/strict";
import { mountLightbox } from "../src/lightbox.js";
import { appDocument } from "./fake-dom.mjs";

function image(doc, name, { ready = true, missing = false, title } = {}) {
  const element = doc.createElement("img");
  element.setAttribute("src", `data:image/png;base64,${name}`);
  element.setAttribute("alt", `Alt ${name}`);
  if (title !== undefined) element.setAttribute("title", title);
  if (ready) element.setAttribute("data-md-asset-ready", "");
  if (missing) element.setAttribute("data-md-asset-missing", "");
  doc.app.appendChild(element);
  return element;
}

function lightbox(doc) {
  return doc.getElementById("mdhtml-lightbox");
}

function control(doc, action) {
  return doc.querySelector(`button[data-md-lightbox-action="${action}"]`);
}

test("opens only ready, non-missing images and copies attributes", () => {
  const doc = appDocument();
  const first = image(doc, "first", { title: "First" });
  const missing = image(doc, "missing", { missing: true });
  const second = image(doc, "second", { title: undefined });
  second.removeAttribute("title");

  assert.equal(mountLightbox(doc, [missing]), null);
  mountLightbox(doc, [first, missing, second]);
  first.click();

  const dialog = lightbox(doc);
  const displayed = dialog.querySelector("img");
  assert.equal(dialog.showModalCalls, 1);
  assert.equal(displayed.getAttribute("src"), first.getAttribute("src"));
  assert.equal(displayed.getAttribute("alt"), "Alt first");
  assert.equal(displayed.getAttribute("title"), "First");
  assert.equal(dialog.querySelector("output[data-md-lightbox-counter]").textContent, "1 / 2");
});

test("recomputes gallery order when a lazy image becomes ready", () => {
  const doc = appDocument();
  const first = image(doc, "first");
  const lazy = image(doc, "lazy", { ready: false });
  const last = image(doc, "last");
  mountLightbox(doc, [first, lazy, last]);

  first.click();
  lazy.setAttribute("data-md-asset-ready", "");
  control(doc, "next").click();

  const dialog = lightbox(doc);
  assert.equal(dialog.querySelector("img").getAttribute("src"), lazy.getAttribute("src"));
  assert.equal(dialog.querySelector("output[data-md-lightbox-counter]").textContent, "2 / 3");
});

test("buttons and arrow keys wrap in both directions", () => {
  const doc = appDocument();
  const first = image(doc, "first");
  const second = image(doc, "second");
  mountLightbox(doc, [first, second]);
  first.click();
  const dialog = lightbox(doc);

  control(doc, "next").click();
  assert.equal(dialog.querySelector("img").getAttribute("src"), second.getAttribute("src"));
  control(doc, "next").click();
  assert.equal(dialog.querySelector("img").getAttribute("src"), first.getAttribute("src"));
  dialog.dispatchEvent("keydown", { key: "ArrowLeft" });
  assert.equal(dialog.querySelector("img").getAttribute("src"), second.getAttribute("src"));
  dialog.dispatchEvent("keydown", { key: "ArrowRight" });
  assert.equal(dialog.querySelector("img").getAttribute("src"), first.getAttribute("src"));
  dialog.dispatchEvent("keydown", { key: "Escape" });
  assert.equal(dialog.closeCalls ?? 0, 0);
});

test("swipe navigates only past the horizontal threshold", () => {
  const doc = appDocument();
  const first = image(doc, "first");
  const second = image(doc, "second");
  const third = image(doc, "third");
  mountLightbox(doc, [first, second, third]);
  second.click();
  const displayed = lightbox(doc).querySelector("img");

  displayed.dispatchEvent("pointerdown", { pointerId: 1, clientX: 100, clientY: 100 });
  displayed.dispatchEvent("pointerup", { pointerId: 1, clientX: 59, clientY: 100 });
  assert.equal(displayed.getAttribute("src"), third.getAttribute("src"));
  displayed.dispatchEvent("pointerdown", { pointerId: 1, clientX: 100, clientY: 100 });
  displayed.dispatchEvent("pointerup", { pointerId: 1, clientX: 141, clientY: 100 });
  assert.equal(displayed.getAttribute("src"), second.getAttribute("src"));
  displayed.dispatchEvent("pointerdown", { pointerId: 1, clientX: 100, clientY: 100 });
  displayed.dispatchEvent("pointerup", { pointerId: 1, clientX: 130, clientY: 100 });
  assert.equal(displayed.getAttribute("src"), second.getAttribute("src"));
  displayed.dispatchEvent("pointerdown", { pointerId: 1, clientX: 100, clientY: 100 });
  displayed.dispatchEvent("pointerup", { pointerId: 1, clientX: 40, clientY: 200 });
  assert.equal(displayed.getAttribute("src"), second.getAttribute("src"));
});

test("mounting twice reuses the dialog and source listeners", () => {
  const doc = appDocument();
  const first = image(doc, "first");
  const second = image(doc, "second");
  const initial = mountLightbox(doc, [first, second]);
  const repeated = mountLightbox(doc, [first, second]);

  assert.equal(repeated, initial);
  assert.equal(doc.querySelectorAll("dialog").length, 1);
  assert.equal(first.listenerCount("click"), 1);
  first.click();
  assert.equal(initial.showModalCalls, 1);
});

test("marks the dialog when reduced motion is preferred", () => {
  const doc = appDocument({ reducedMotion: true });
  mountLightbox(doc, [image(doc, "first")]);

  assert.deepEqual(doc.defaultView.mediaQueries, ["(prefers-reduced-motion: reduce)"]);
  assert.equal(lightbox(doc).getAttribute("data-md-reduced-motion"), "");
});
