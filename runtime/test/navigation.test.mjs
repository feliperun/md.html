import { test } from "node:test";
import assert from "node:assert/strict";
import { mountToc } from "../src/navigation.js";
import { appDocument } from "./fake-dom.mjs";

function toolbarDocument() {
  const doc = appDocument();
  const toolbar = doc.createElement("nav");
  toolbar.setAttribute("id", "mdhtml-toolbar");
  doc.body.insertBefore(toolbar, doc.getElementById("mdhtml-app"));
  return doc;
}

test("mountToc creates a flat native TOC in heading order", () => {
  const doc = toolbarDocument();
  const headings = [
    { level: 1, id: "intro", text: "Intro" },
    { level: 3, id: "details", text: "Details" },
  ];

  const toc = mountToc(doc, headings, { depth: 3, position: "inline" });

  assert.equal(toc.tagName, "NAV");
  assert.equal(toc.id, "mdhtml-toc");
  assert.equal(toc.getAttribute("aria-label"), "Table of contents");
  assert.equal(doc.body.children[1], toc);
  assert.equal(toc.children[0].tagName, "OL");
  assert.deepEqual(toc.children[0].children.map((item) => item.children[0].getAttribute("href")), ["#intro", "#details"]);
  assert.deepEqual(toc.children[0].children.map((item) => item.children[0].textContent), ["Intro", "Details"]);
  assert.deepEqual(toc.children[0].children.map((item) => item.children[0].getAttribute("data-level")), ["1", "3"]);
  assert.equal(toc.getAttribute("data-md-toc-position"), "inline");
});

test("mountToc applies the closed toc configuration", () => {
  const doc = toolbarDocument();
  const headings = [
    { level: 1, id: "one", text: "One" },
    { level: 3, id: "three", text: "Three" },
    { level: 6, id: "six", text: "Six" },
  ];

  assert.equal(mountToc(doc, headings, false), null);
  const toc = mountToc(doc, headings, { depth: 2, position: "unknown" });
  assert.equal(toc.querySelectorAll("a").length, 1);
  assert.equal(toc.getAttribute("data-md-toc-position"), "side");
});

test("mountToc omits a configured map when no heading survives its depth", () => {
  const doc = toolbarDocument();
  assert.equal(mountToc(doc, [{ level: 4, id: "deep", text: "Deep" }], { depth: 3 }), null);
  assert.equal(doc.getElementById("mdhtml-toc"), null);
});

test("mountToc is harmless when its toolbar dependency is absent", () => {
  const doc = appDocument();
  assert.doesNotThrow(() => mountToc(doc, [{ level: 1, id: "intro", text: "Intro" }]));
  assert.equal(doc.getElementById("mdhtml-toc"), null);
});

test("mountToc synchronizes decoded hashes and tolerates malformed encoding", () => {
  const doc = toolbarDocument();
  const toc = mountToc(doc, [{ level: 1, id: "résumé", text: "Résumé" }, { level: 2, id: "other", text: "Other" }]);
  const links = toc.querySelectorAll("a");

  doc.defaultView.location.hash = "#r%C3%A9sum%C3%A9";
  doc.defaultView.dispatchEvent("hashchange");
  assert.equal(links[0].getAttribute("aria-current"), "location");
  assert.equal(links[1].getAttribute("aria-current"), null);

  doc.defaultView.location.hash = "%E0%A4%A";
  doc.defaultView.dispatchEvent("hashchange");
  assert.equal(links[0].getAttribute("aria-current"), null);
  assert.equal(links[1].getAttribute("aria-current"), null);
});

test("mountToc is empty-safe and idempotent", () => {
  const doc = toolbarDocument();
  assert.equal(mountToc(doc, []), null);
  const headings = [{ level: 1, id: "intro", text: "Intro" }];
  const first = mountToc(doc, headings);
  const second = mountToc(doc, headings);
  assert.equal(second, first);
  assert.equal(doc.body.children.filter((child) => child.id === "mdhtml-toc").length, 1);
  assert.equal(doc.defaultView.listenerCount("hashchange"), 1);
});
