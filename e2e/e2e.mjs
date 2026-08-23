import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "playwright";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const TARGET_DIR = join(ROOT, ".runs", "cargo-t16");
const BIN = join(TARGET_DIR, "debug", "mdhtml");
const EXAMPLE_SOURCE = join(ROOT, "examples", "spec.md");

const LAZY_HYDRATION_THRESHOLD = 32768;

function runBinary(args) {
  return execFileSync(BIN, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
}

function ensureBinary() {
  try {
    execFileSync(
      "cargo",
      [
        "build",
        "--locked",
        "--target-dir",
        TARGET_DIR,
        "--manifest-path",
        join(ROOT, "Cargo.toml"),
        "-p",
        "mdhtml",
      ],
      { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
    );
  } catch (error) {
    process.stderr.write(String(error.stderr ?? error.message));
    throw new Error("mdhtml binary build failed");
  }
}

function syntheticDiagram() {
  const parts = [
    '<svg xmlns="http://www.w3.org/2000/svg" width="1140" height="660" viewBox="0 0 1140 660" font-family="ui-monospace, monospace" font-size="12">',
  ];
  for (let row = 0; row < 20; row += 1) {
    for (let column = 0; column < 10; column += 1) {
      const x = 30 + column * 112;
      const y = 30 + row * 32;
      const fill = (row + column) % 2 === 0 ? "#dbeafe" : "#e0e7ff";
      parts.push(
        `<rect x="${x}" y="${y}" width="96" height="20" rx="4" fill="${fill}" stroke="#6366f1" stroke-width="1.5"/>`,
      );
      parts.push(
        `<text x="${x + 48}" y="${y + 14}" text-anchor="middle" fill="#0f172a">svc-${String(row).padStart(2, "0")}-${String(column).padStart(2, "0")}</text>`,
      );
    }
  }
  parts.push("</svg>");
  const svg = parts.join("\n");
  assert.ok(
    Buffer.byteLength(svg) >= LAZY_HYDRATION_THRESHOLD,
    "the E2E diagram must stay at or above the lazy-hydration threshold",
  );
  return svg;
}

function withDiagram(source) {
  const marker = "# Meridian API\n";
  const index = source.indexOf(marker);
  assert.notEqual(index, -1, "examples/spec.md must open with the # Meridian API heading");
  const at = index + marker.length;
  return `${source.slice(0, at)}\n![Meridian service architecture](diagram.svg)\n${source.slice(at)}`;
}

function sourceHeadings(source) {
  return source
    .split("\n")
    .filter((line) => /^#{1,6} /.test(line))
    .map((line) => line.replace(/^#{1,6} /, "").replace(/\s*\{#[^}]+\}\s*$/, "").trim());
}

async function main() {
  ensureBinary();

  const work = mkdtempSync(join(tmpdir(), "mdhtml-e2e-"));
  try {
    const source = withDiagram(readFileSync(EXAMPLE_SOURCE, "utf8"));
    writeFileSync(join(work, "spec-e2e.md"), source);
    // The example declares `theme: spec.theme.css` in its front matter; the
    // build resolves it next to the source, so it must travel with the copy.
    writeFileSync(
      join(work, "spec.theme.css"),
      readFileSync(join(ROOT, "examples", "spec.theme.css")),
    );
    writeFileSync(join(work, "diagram.svg"), syntheticDiagram());
    runBinary(["build", join(work, "spec-e2e.md"), "-o", join(work, "spec-e2e.md.html")]);
    const documentUrl = pathToFileURL(join(work, "spec-e2e.md.html")).href;

    const browser = await chromium.launch({ headless: true });
    try {
      const context = await browser.newContext({ acceptDownloads: true });
      const page = await context.newPage();

      const requests = [];
      page.on("request", (request) => requests.push(request.url()));
      await page.goto(documentUrl, { waitUntil: "load" });
      await page.waitForFunction(() =>
        document.documentElement.hasAttribute("data-mdhtml-ready"));

      const expectedHeadings = sourceHeadings(source);

      // 1. Rendered sections match the canonical source.
      const renderedHeadings = await page.$$eval(
        "#mdhtml-app h1, #mdhtml-app h2, #mdhtml-app h3, #mdhtml-app h4, #mdhtml-app h5, #mdhtml-app h6",
        (elements) => elements.map((element) => element.textContent.trim()),
      );
      assert.deepEqual(
        renderedHeadings,
        expectedHeadings,
        "rendered headings must match the canonical source in order",
      );
      assert.equal(
        await page.$$eval("#mdhtml-app section[data-md-section]", (elements) => elements.length),
        expectedHeadings.length,
        "every heading must be an addressable section",
      );
      const sectionHeadingIds = await page.$$eval(
        "#mdhtml-app h1[id], #mdhtml-app h2[id], #mdhtml-app h3[id]",
        (elements) => elements.map((element) => element.id),
      );
      assert.equal(
        new Set(sectionHeadingIds).size,
        sectionHeadingIds.length,
        "heading ids must be unique",
      );
      const tocHeadingIds = await page.$$eval(
        "#mdhtml-app h1[id], #mdhtml-app h2[id]",
        (elements) => elements.map((element) => element.id),
      );
      const tocLinks = await page.$$eval("#mdhtml-toc a", (elements) =>
        elements.map((element) => element.getAttribute("href")));
      assert.deepEqual(
        tocLinks,
        tocHeadingIds.map((id) => `#${id}`),
        "the TOC must carry one anchor per in-depth heading",
      );
      assert.equal(
        await page.$eval("#mdhtml-app", (element) =>
          element.textContent.includes("single source of truth")),
        true,
        "the rendered body must include the canonical prose",
      );

      // 2. Hash navigation updates the active TOC link.
      await page.click('a[href="#goals"]');
      await page.waitForFunction(() =>
        document.querySelector('a[href="#goals"]')?.getAttribute("aria-current") === "location");
      assert.equal(
        await page.$$eval('#mdhtml-toc a[aria-current="location"]', (elements) => elements.length),
        1,
        "exactly one TOC link is active after hash navigation",
      );
      await page.evaluate(() => {
        location.hash = "#no-such-section";
      });
      await page.waitForFunction(() =>
        document.querySelectorAll('#mdhtml-toc a[aria-current="location"]').length === 0);
      assert.equal(
        await page.$$eval('#mdhtml-toc a[aria-current="location"]', (elements) => elements.length),
        0,
        "an unknown hash must deactivate every TOC link",
      );

      // 3. Clipboard copy falls back without a network call.
      await page.selectOption("#mdhtml-copy-mode", "full");
      await page.evaluate(() => {
        window.__copiedText = null;
        Object.defineProperty(navigator, "clipboard", {
          configurable: true,
          value: { writeText: () => Promise.reject(new Error("denied")) },
        });
        document.execCommand = (command) => {
          if (command === "copy") {
            const textarea = document.querySelector("body textarea");
            window.__copiedText = textarea ? textarea.value : null;
            return true;
          }
          return false;
        };
      });
      await page.click('[data-md-action="copy"]');
      assert.equal(
        await page.evaluate(() => window.__copiedText),
        source,
        "the clipboard fallback must copy the exact canonical source",
      );

      // 4. The download action produces the byte-exact source.
      const [download] = await Promise.all([
        page.waitForEvent("download"),
        page.click('[data-md-action="download"]'),
      ]);
      const downloadPath = await download.path();
      assert.ok(downloadPath, "the download action must produce a file path");
      assert.deepEqual(
        readFileSync(downloadPath),
        Buffer.from(source, "utf8"),
        "the download must be byte-identical to the canonical source",
      );

      // 5. The theme cycle toggles light/dark/system.
      const theme = () =>
        page.evaluate(() => document.documentElement.getAttribute("data-mdhtml-theme"));
      assert.equal(await theme(), "system", "a fresh document starts in system theme");
      for (const expected of ["light", "dark", "system"]) {
        await page.click('[data-md-action="theme"]');
        assert.equal(await theme(), expected, `theme cycle step must reach ${expected}`);
      }

      // 6. Image hydration renders an embedded asset.
      await page.$eval('img[data-md-asset-path="diagram.svg"]', (image) => image.scrollIntoView());
      await page.waitForFunction(() => {
        const image = document.querySelector('img[data-md-asset-path="diagram.svg"]');
        return image !== null && image.hasAttribute("data-md-asset-ready") && image.getAttribute("src") !== "";
      });
      const imageState = await page.$eval('img[data-md-asset-path="diagram.svg"]', (image) => ({
        src: image.getAttribute("src"),
        ready: image.hasAttribute("data-md-asset-ready"),
        width: image.naturalWidth,
      }));
      assert.ok(imageState.ready, "the embedded image must hydrate");
      assert.ok(
        imageState.src.startsWith("blob:") || imageState.src.startsWith("data:"),
        "the hydrated image must use an inline source",
      );
      assert.ok(imageState.width > 0, "the hydrated image must actually render");
      assert.ok(
        imageState.src.startsWith("blob:"),
        "a payload at or above 32 KiB must hydrate through a Blob URL",
      );

      // 7. The document itself performs zero network requests.
      const unexpected = requests.filter(
        (url) => url !== documentUrl && !url.startsWith("blob:"));
      assert.deepEqual(
        unexpected,
        [],
        `the document must make zero network requests (got: ${unexpected.join(", ")})`,
      );

      await context.close();

      // 8. Without JavaScript the noscript fallback shows the canonical source.
      const noJsContext = await browser.newContext({ javaScriptEnabled: false });
      const noJsPage = await noJsContext.newPage();
      await noJsPage.goto(documentUrl, { waitUntil: "load" });
      const fallback = await noJsPage.$eval("#mdhtml-source", (element) => ({
        display: getComputedStyle(element).display,
        height: element.getBoundingClientRect().height,
        text: element.textContent,
      }));
      assert.notEqual(fallback.display, "none", "the noscript fallback must be visible");
      assert.ok(fallback.height > 0, "the noscript fallback must occupy space");
      assert.equal(
        fallback.text,
        source,
        "the noscript fallback must show the canonical source",
      );
      await noJsContext.close();
    } finally {
      await browser.close().catch(() => {});
    }

    console.log(`e2e: all scenarios passed for ${documentUrl}`);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(`e2e: ${error.message}`);
  process.exitCode = 1;
});
