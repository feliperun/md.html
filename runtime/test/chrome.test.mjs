import { test } from "node:test";
import assert from "node:assert/strict";
import { mountChrome, copyText } from "../src/chrome.js";
import { projectMarkdown } from "../src/canonical.js";
import { loadFixtures } from "./fixtures.mjs";
import { appDocument } from "./fake-dom.mjs";

function action(doc, name) {
  return doc.find(doc.documentElement, `button[data-md-action="${name}"]`);
}

test("mountChrome creates ordered native controls and is idempotent", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "browser-chrome");
  const doc = appDocument();
  const chrome = mountChrome(doc, fixture.source, projectMarkdown);

  assert.equal(chrome.tagName, "NAV");
  assert.equal(chrome.id, "mdhtml-toolbar");
  assert.equal(chrome.getAttribute("aria-label"), "Document controls");
  assert.deepEqual(chrome.children.map((child) => child.tagName), ["SELECT", "BUTTON", "BUTTON", "BUTTON", "BUTTON"]);
  assert.deepEqual(chrome.children[0].children.map((child) => child.value), ["smart", "full", "body"]);
  assert.equal(doc.body.children[0], chrome);
  assert.equal(doc.body.children[1].id, "mdhtml-source-view");
  assert.equal(mountChrome(doc, "different source", projectMarkdown), chrome);
  assert.equal(doc.body.children.filter((child) => child.id === "mdhtml-toolbar").length, 1);
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-theme"), "system");
});

test("copy calls clipboard in the click stack and uses the current projection", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "browser-chrome");
  let inClick = false;
  let calledInClick = false;
  let copied = "";
  const doc = appDocument();
  doc.defaultView.navigator.clipboard = {
    writeText(text) {
      calledInClick = inClick;
      copied = text;
      return Promise.resolve();
    },
  };
  mountChrome(doc, fixture.source, projectMarkdown);
  const mode = doc.getElementById("mdhtml-copy-mode");
  mode.value = "body";
  inClick = true;
  action(doc, "copy").click();
  inClick = false;

  assert.equal(calledInClick, true);
  assert.equal(copied, projectMarkdown(fixture.source, "body"));
});

test("clipboard rejection falls back to a readonly fixed textarea and cleans it up", async () => {
  const doc = appDocument();
  let reject;
  doc.defaultView.navigator.clipboard = { writeText: () => new Promise((resolve, rejectPromise) => { reject = rejectPromise; }) };
  copyText(doc, "fallback text");
  reject(new Error("denied"));
  await Promise.resolve();
  const textarea = doc.created.find((entry) => entry.tagName === "TEXTAREA");
  assert.equal(textarea.attributes.readonly, "");
  assert.equal(textarea.style.position, "fixed");
  assert.equal(textarea.value, "fallback text");
  assert.equal(textarea.selected, true);
  assert.equal(doc.command, "copy");
  assert.equal(textarea.parentNode, null);
});

test("view uses the selected projection and close is native", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "browser-chrome");
  const doc = appDocument();
  mountChrome(doc, fixture.source, projectMarkdown);
  const mode = doc.getElementById("mdhtml-copy-mode");
  mode.value = "full";
  action(doc, "view").click();
  const dialog = doc.getElementById("mdhtml-source-view");
  assert.equal(dialog.querySelector("pre").textContent, fixture.source);
  assert.equal(dialog.showModalCalled, true);
  dialog.querySelector("button").click();
  assert.equal(dialog.closeCalled, true);
});

test("download uses the full stored source and revokes its temporary URL", async () => {
  const fixture = (await loadFixtures()).find((entry) => entry.id === "browser-chrome");
  const doc = appDocument();
  mountChrome(doc, fixture.source, projectMarkdown);
  action(doc, "download").click();

  const anchor = doc.created.find((entry) => entry.tagName === "A");
  assert.equal(anchor.getAttribute("download"), "document.md");
  assert.equal(anchor.getAttribute("href"), "blob:document");
  assert.equal(doc.defaultView.URL.blob.parts[0], fixture.source);
  assert.equal(doc.defaultView.URL.revoked, "blob:document");
  assert.equal(anchor.parentNode, null);
});

test("theme cycles system, light, dark without persistence", () => {
  const doc = appDocument();
  mountChrome(doc, "# Theme\n", projectMarkdown);
  const theme = action(doc, "theme");
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-theme"), "system");
  theme.click();
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-theme"), "light");
  theme.click();
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-theme"), "dark");
  theme.click();
  assert.equal(doc.documentElement.getAttribute("data-mdhtml-theme"), "system");
});
