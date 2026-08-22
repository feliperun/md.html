// Dependency-free HTML serialization for the accepted Markdown AST.

import { parseInline, parseMarkdown } from "./markdown.js";
import { orderedMappingEntries } from "./frontmatter.js";

const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);

function escapeText(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeAttribute(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function slugify(value) {
  return String(value)
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .replace(/\s+/gu, "-")
    .replace(/[^A-Za-z0-9_-]/g, "");
}

function headingParts(block) {
  const match = /\s+\{#([^}\n]+)\}$/u.exec(block.text);
  if (!match) return { text: block.text, requestedId: null };
  return {
    text: block.text.slice(0, match.index).replace(/[ \t]+$/u, ""),
    requestedId: match[1].trim().toLowerCase().replace(/\s+/gu, "-"),
  };
}

function inlineText(nodes) {
  let value = "";
  for (const node of nodes ?? []) {
    if (node.type === "text" || node.type === "code") value += node.value;
    else if (node.type === "image") value += node.alt;
    else if (node.type === "link" || node.type === "emphasis" || node.type === "strong" || node.type === "strike") {
      value += inlineText(node.children);
    }
  }
  return value;
}

function inlineNodes(block, context) {
  return Array.isArray(block.children) ? block.children : parseInline(block.text, context.references);
}

function addFootnoteReference(id, context) {
  if (!context.footnoteSeen.has(id)) {
    context.footnoteSeen.add(id);
    context.footnoteOrder.push(id);
  }
}

function renderInline(nodes, context) {
  let html = "";
  for (const node of nodes ?? []) {
    switch (node.type) {
      case "text":
        html += escapeText(node.value);
        break;
      case "code":
        html += `<code>${escapeText(node.value)}</code>`;
        break;
      case "emphasis":
        html += `<em>${renderInline(node.children, context)}</em>`;
        break;
      case "strong":
        html += `<strong>${renderInline(node.children, context)}</strong>`;
        break;
      case "strike":
        html += `<del>${renderInline(node.children, context)}</del>`;
        break;
      case "link": {
        const title = node.title === null || node.title === undefined
          ? ""
          : ` title="${escapeAttribute(node.title)}"`;
        html += `<a href="${escapeAttribute(node.url)}"${title}>${renderInline(node.children, context)}</a>`;
        break;
      }
      case "image": {
        const title = node.title === null || node.title === undefined
          ? ""
          : ` title="${escapeAttribute(node.title)}"`;
        html += `<img data-md-asset-path="${escapeAttribute(node.src)}" alt="${escapeAttribute(node.alt)}"${title}>`;
        break;
      }
      case "hardBreak":
        html += "<br>";
        break;
      case "footnoteReference":
        addFootnoteReference(node.id, context);
        html += `<sup><a href="#fn-${escapeAttribute(node.id)}">[${escapeText(node.id)}]</a></sup>`;
        break;
      default:
        if (node.value !== undefined) html += escapeText(node.value);
        else if (node.children !== undefined) html += renderInline(node.children, context);
    }
  }
  return html;
}

function renderHeading(block, context) {
  const heading = registerHeading(block, context);
  return {
    html: `<section data-md-section="${escapeAttribute(heading.id)}"><h${block.level} id="${escapeAttribute(heading.id)}">${renderInline(heading.children, context)}</h${block.level}>`,
    level: heading.level,
  };
}

function registerHeading(block, context) {
  const parts = headingParts(block);
  const children = parts.text === block.text && Array.isArray(block.children)
    ? block.children
    : parseInline(parts.text, context.references);
  const text = inlineText(children);
  const base = parts.requestedId === null ? slugify(text) : parts.requestedId;
  if (parts.requestedId !== null && context.usedIds.has(base)) {
    context.warnings.push({ code: "W-SECT-01" });
  }
  let id = base;
  let suffix = 2;
  while (context.usedIds.has(id)) id = `${base}-${suffix++}`;
  context.usedIds.add(id);
  context.headings.push({ level: block.level, id, text });
  return { children, id, level: block.level };
}

function renderTable(block, context) {
  const cell = (tag, value, align) => {
    const attribute = align === null || align === undefined ? "" : ` align="${escapeAttribute(align)}"`;
    return `<${tag}${attribute}>${renderInline(value, context)}</${tag}>`;
  };
  const header = block.headerInlines ?? block.header.map((value) => parseInline(value, context.references));
  const rows = block.rowInlines ?? block.rows.map((row) => row.map((value) => parseInline(value, context.references)));
  return `<table><thead><tr>${header.map((value, i) => cell("th", value, block.align[i])).join("")}</tr></thead><tbody>${rows.map((row) => `<tr>${row.map((value, i) => cell("td", value, block.align[i])).join("")}</tr>`).join("")}</tbody></table>`;
}

const CALLOUT_NAMES = new Set(["note", "warning", "critical", "success", "decision"]);

function renderContainerArgument(argument, context) {
  return renderInline(parseInline(argument, context.references), context);
}

function degradeContainer(block, context) {
  context.warnings.push({ code: "W-COMP-02", name: block.name, target: null });
  return renderBlocks(block.children, context);
}

function hasExactTwoColumnEvidence(block) {
  const counts = block?.sourceCellCounts;
  return counts?.header === 2 &&
    Array.isArray(counts.rows) &&
    counts.rows.length === block.rows.length &&
    counts.rows.every((count) => count === 2);
}

function isTwoColumnTable(block) {
  return block?.type === "table" &&
    Array.isArray(block.header) &&
    block.header.length === 2 &&
    Array.isArray(block.rows) &&
    block.rows.length > 0 &&
    hasExactTwoColumnEvidence(block) &&
    block.rows.every((row) => Array.isArray(row) && row.length === 2);
}

function getSingleTwoColumnTable(children) {
  return children.length === 1 && isTwoColumnTable(children[0]) ? children[0] : null;
}

function renderBarsTable(block, values, max, context) {
  const cell = (tag, value, align) => {
    const attribute = align === null || align === undefined ? "" : ` align="${escapeAttribute(align)}"`;
    return `<${tag}${attribute}>${renderInline(value, context)}</${tag}>`;
  };
  const header = block.headerInlines ?? block.header.map((value) => parseInline(value, context.references));
  const rows = block.rowInlines ?? block.rows.map((row) => row.map((value) => parseInline(value, context.references)));
  return `<table><thead><tr>${header.map((value, i) => cell("th", value, block.align[i])).join("")}</tr></thead><tbody>${rows.map((row, i) => `<tr>${cell("td", row[0], block.align[0])}<td><meter min="0" max="${escapeAttribute(max)}" value="${escapeAttribute(values[i])}">${escapeText(values[i])}</meter></td></tr>`).join("")}</tbody></table>`;
}

function parseBarValue(value) {
  const trimmed = String(value).trim();
  if (!/^[+\-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/u.test(trimmed)) return null;
  const number = Number(trimmed);
  return Number.isFinite(number) && number >= 0 ? number : null;
}

function renderStats(block, context) {
  const table = getSingleTwoColumnTable(block.children);
  if (block.argument !== null || table === null) return null;
  return `<div class="md-stats">${renderTable(table, context)}</div>`;
}

function renderBars(block, context) {
  const table = getSingleTwoColumnTable(block.children);
  if (block.argument !== null || table === null) return null;
  const values = table.rows.map((row) => parseBarValue(row[1]));
  if (values.some((value) => value === null)) return null;
  const max = Math.max(...values, 0) || 1;
  return `<div class="md-bars">${renderBarsTable(table, values.map(String), String(max), context)}</div>`;
}

function renderKeyValueRows(rows, context) {
  return `<dl class="md-kv">${rows.map(([key, value]) => `<dt>${renderInline(key, context)}</dt><dd>${renderInline(value, context)}</dd>`).join("")}</dl>`;
}

function renderKeyValueTable(block, context) {
  const rows = block.rowInlines ?? block.rows.map((row) => row.map((value) => parseInline(value, context.references)));
  return renderKeyValueRows(rows.map((row) => [row[0], row[1]]), context);
}

function getKeyValueListEntries(list) {
  if (list?.type !== "list" || list.ordered || list.items.length === 0) return null;
  const entries = [];
  for (const item of list.items) {
    if ((item.checked !== null && item.checked !== undefined) || item.children.length === 0) return null;
    const first = item.children[0];
    const inlines = first?.type === "paragraph" && Array.isArray(first.children) ? first.children : null;
    if (inlines === null || inlines[0]?.type !== "strong" || inlines[1]?.type !== "text" || !inlines[1].value.startsWith(":")) return null;
    const valueText = inlines[1].value.slice(1).replace(/^[ \t]/u, "");
    entries.push({ key: inlines[0].children, value: [{ ...inlines[1], value: valueText }, ...inlines.slice(2)], blocks: item.children.slice(1) });
  }
  return entries;
}

function renderKeyValueList(list, context) {
  const entries = getKeyValueListEntries(list);
  if (entries === null) return null;
  return `<dl class="md-kv">${entries.map((entry) => `<dt>${renderInline(entry.key, context)}</dt><dd>${renderInline(entry.value, context)}${renderBlocks(entry.blocks, context)}</dd>`).join("")}</dl>`;
}

function renderKeyValue(block, context) {
  if (block.argument !== null || block.children.length !== 1) return null;
  const table = getSingleTwoColumnTable(block.children);
  if (table !== null) return renderKeyValueTable(table, context);
  return renderKeyValueList(block.children[0], context);
}

function isNonemptyOrderedList(block) {
  return block?.type === "list" && block.ordered && block.items.length > 0 && block.items.every((item) => item.children.length > 0);
}

function renderStepItem(item, context) {
  const task = item.checked === null || item.checked === undefined
    ? ""
    : `<input type="checkbox" disabled${item.checked ? " checked" : ""}> `;
  if (item.children.length === 1 && item.children[0].type === "paragraph") {
    return `<li>${task}${renderInline(inlineNodes(item.children[0], context), context)}</li>`;
  }
  return `<li>${task}${renderBlocks(item.children, context)}</li>`;
}

function renderSteps(block, context) {
  if (block.argument !== null || block.children.length !== 1 || !isNonemptyOrderedList(block.children[0])) return null;
  const list = block.children[0];
  const start = list.start !== 1 ? ` start="${escapeAttribute(list.start)}"` : "";
  return `<ol class="md-steps"${start}>${list.items.map((item) => renderStepItem(item, context)).join("")}</ol>`;
}

function renderGrid(block, context) {
  if (block.argument !== null || block.children.length === 0 || block.children[0].type !== "heading" || block.children[0].level !== 3) return null;
  const groups = [];
  let group = null;
  for (const child of block.children) {
    if (child.type === "heading" && child.level === 3) {
      if (group !== null) groups.push(group);
      group = [child];
    } else if (group === null) {
      return null;
    } else {
      group.push(child);
    }
  }
  if (group !== null) groups.push(group);
  return `<div class="md-grid">${groups.map((items) => {
    const heading = registerHeading(items[0], context);
    return `<section class="md-grid-item" data-md-section="${escapeAttribute(heading.id)}"><h3 id="${escapeAttribute(heading.id)}">${renderInline(heading.children, context)}</h3>${renderBlocks(items.slice(1), context)}</section>`;
  }).join("")}</div>`;
}

function renderContainer(block, context) {
  const children = Array.isArray(block.children) ? block.children : [];
  const argument = block.argument;

  if (block.name === "stats") {
    const html = renderStats({ ...block, children }, context);
    return html ?? degradeContainer(block, context);
  }
  if (block.name === "bars") {
    const html = renderBars({ ...block, children }, context);
    return html ?? degradeContainer(block, context);
  }
  if (block.name === "kv") {
    const html = renderKeyValue({ ...block, children }, context);
    return html ?? degradeContainer(block, context);
  }
  if (block.name === "steps") {
    const html = renderSteps({ ...block, children }, context);
    return html ?? degradeContainer(block, context);
  }
  if (block.name === "grid") {
    const html = renderGrid({ ...block, children }, context);
    return html ?? degradeContainer(block, context);
  }

  if (CALLOUT_NAMES.has(block.name)) {
    if (argument !== null || children.length === 0) return degradeContainer(block, context);
    const label = `${block.name[0].toUpperCase()}${block.name.slice(1)}`;
    return `<aside class="md-callout md-${block.name}"><span class="md-callout-badge">${label}</span>${renderBlocks(children, context)}</aside>`;
  }

  if (block.name === "quote") {
    if (children.length === 0) return degradeContainer(block, context);
    const attribution = argument === null ? "" : `<figcaption>${renderContainerArgument(argument, context)}</figcaption>`;
    return `<figure class="md-quote"><blockquote>${renderBlocks(children, context)}</blockquote>${attribution}</figure>`;
  }

  if (block.name === "columns") {
    if (argument !== null || children.length < 2) return degradeContainer(block, context);
    return `<div class="md-columns">${children.map((child) => `<div class="md-column">${renderBlocks([child], context)}</div>`).join("")}</div>`;
  }

  if (block.name === "details") {
    if (children.length === 0) return degradeContainer(block, context);
    const summary = argument === null ? "Details" : argument;
    return `<details class="md-details"><summary>${renderContainerArgument(summary, context)}</summary>${renderBlocks(children, context)}</details>`;
  }

  return degradeContainer(block, context);
}

function renderBlock(block, context) {
  if (block.type === "paragraph") return `<p>${renderInline(inlineNodes(block, context), context)}</p>`;
  if (block.type === "codeBlock") {
    const className = block.language === null || block.language === undefined
      ? ""
      : ` class="language-${escapeAttribute(block.language)}"`;
    return `<pre><code${className}>${escapeText(block.value)}</code></pre>`;
  }
  if (block.type === "blockquote") return `<blockquote>${renderBlocks(block.children, context)}</blockquote>`;
  if (block.type === "list") {
    const tag = block.ordered ? "ol" : "ul";
    const start = block.ordered && block.start !== 1 ? ` start="${escapeAttribute(block.start)}"` : "";
    const items = block.items.map((item) => {
      const task = item.checked === null || item.checked === undefined
        ? ""
        : `<input type="checkbox" disabled${item.checked ? " checked" : ""}> `;
      return `<li>${task}${renderBlocks(item.children, context)}</li>`;
    }).join("");
    return `<${tag}${start}>${items}</${tag}>`;
  }
  if (block.type === "table") return renderTable(block, context);
  if (block.type === "thematicBreak") return "<hr>";
  if (block.type === "container") return renderContainer(block, context);
  return "";
}

function renderBlocks(blocks, context) {
  let html = "";
  const stack = [];
  for (const block of blocks ?? []) {
    if (block.type === "heading") {
      while (stack.length > 0 && block.level <= stack[stack.length - 1]) {
        html += "</section>";
        stack.pop();
      }
      const heading = renderHeading(block, context);
      html += heading.html;
      stack.push(heading.level);
    } else {
      html += renderBlock(block, context);
    }
  }
  while (stack.length > 0) {
    html += "</section>";
    stack.pop();
  }
  return html;
}

const SECTION_COMPONENTS = new Set(["timeline", "cards", "meters", "gallery", "kv", "columns", "hero"]);
const CLASS_RE = /^(?:[A-Za-z_][A-Za-z0-9_-]*)(?:\s+[A-Za-z_][A-Za-z0-9_-]*)*$/u;

function isPlainMapping(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function groupSections(blocks) {
  const roots = [];
  const stack = [];
  for (const block of blocks ?? []) {
    if (block.type !== "heading") {
      if (stack.length === 0) roots.push(block);
      else stack[stack.length - 1].body.push(block);
      continue;
    }
    while (stack.length > 0 && block.level <= stack[stack.length - 1].heading.level) stack.pop();
    const section = { type: "section", heading: block, body: [] };
    if (stack.length === 0) roots.push(section);
    else stack[stack.length - 1].body.push(section);
    stack.push(section);
  }
  return roots;
}

function renderSectionBodyNode(node, context) {
  return node.type === "section" ? renderSectionNode(node, context) : renderBlock(node, context);
}

function renderSectionBody(nodes, context) {
  return (nodes ?? []).map((node) => renderSectionBodyNode(node, context)).join("");
}

function renderCompactList(block, context) {
  const tag = block.ordered ? "ol" : "ul";
  const start = block.ordered && block.start !== 1 ? ` start="${escapeAttribute(block.start)}"` : "";
  return `<${tag}${start}>${block.items.map((item) => renderStepItem(item, context)).join("")}</${tag}>`;
}

function renderSectionMeters(block, values, context) {
  const cell = (tag, value, align) => {
    const attribute = align === null || align === undefined ? "" : ` align="${escapeAttribute(align)}"`;
    return `<${tag}${attribute}>${renderInline(value, context)}</${tag}>`;
  };
  const header = block.headerInlines ?? block.header.map((value) => parseInline(value, context.references));
  const rows = block.rowInlines ?? block.rows.map((row) => row.map((value) => parseInline(value, context.references)));
  return `<table><thead><tr>${header.map((value, i) => cell("th", value, block.align[i])).join("")}</tr></thead><tbody>${rows.map((row, i) => `<tr>${cell("td", row[0], block.align[0])}<td><meter min="0" max="100" value="${escapeAttribute(values[i])}">${escapeText(values[i])}</meter></td></tr>`).join("")}</tbody></table>`;
}

function isSectionImageParagraph(block) {
  const nodes = block?.type === "paragraph" ? block.children : null;
  return Array.isArray(nodes) && nodes.length === 1 && nodes[0].type === "image";
}

function renderSectionComponent(name, body, context) {
  if (name === "timeline") {
    if (body.length !== 1 || body[0].type !== "list" || body[0].items.length === 0 || body[0].items.some((item) => item.children.length === 0)) return null;
    return `<div class="md-timeline">${renderCompactList(body[0], context)}</div>`;
  }
  if (name === "cards") {
    if (body.length === 0 || body.some((node) => node.type !== "section")) return null;
    return `<div class="md-cards">${body.map((node) => renderSectionNode(node, context)).join("")}</div>`;
  }
  if (name === "meters") {
    const table = getSingleTwoColumnTable(body);
    if (table === null) return null;
    const values = table.rows.map((row) => parseBarValue(row[1]));
    if (values.some((value) => value === null || value > 100)) return null;
    return `<div class="md-meters">${renderSectionMeters(table, values.map(String), context)}</div>`;
  }
  if (name === "gallery") {
    if (body.length === 0 || body.some((node) => !isSectionImageParagraph(node))) return null;
    return `<div class="md-gallery">${body.map((node) => `<figure class="md-gallery-item">${renderBlock(node, context)}</figure>`).join("")}</div>`;
  }
  if (name === "kv") {
    return renderKeyValue({ argument: null, children: body }, context);
  }
  if (name === "columns") {
    if (body.length < 2) return null;
    return `<div class="md-columns">${body.map((node) => `<div class="md-column">${renderSectionBodyNode(node, context)}</div>`).join("")}</div>`;
  }
  if (name === "hero") {
    if (body.length === 0) return null;
    const media = body.filter((node) => isSectionImageParagraph(node));
    if (media.length > 1) return null;
    const content = body.filter((node) => !isSectionImageParagraph(node));
    return `<div class="md-hero"><div class="md-hero-content">${renderSectionBody(content, context)}</div><div class="md-hero-media">${media.length === 1 ? renderBlock(media[0], context) : ""}</div></div>`;
  }
  return null;
}

function prepareSectionBindings(sections, context) {
  context.sectionBindings = Object.create(null);
  context.bindingRecords = [];
  if (!isPlainMapping(sections)) return;
  for (const [slug, raw] of orderedMappingEntries(sections)) {
    const mapping = isPlainMapping(raw);
    const component = mapping && typeof raw.component === "string" ? raw.component : "";
    const hasClass = mapping && own(raw, "class");
    const classValue = hasClass && typeof raw.class === "string" ? raw.class : null;
    const shapeValid = mapping && typeof raw.component === "string" &&
      (!hasClass || typeof raw.class === "string") &&
      (!hasClass || CLASS_RE.test(raw.class));
    const record = { slug, component, class: classValue, valid: false };
    const meta = { slug, component, classValue, hasClass, shapeValid, record };
    context.sectionBindings[slug] = meta;
    context.bindingRecords.push(meta);
  }
}

function sectionWarning(meta) {
  return { code: "W-COMP-02", name: meta.shapeValid ? meta.component : (meta.component || ""), target: meta.slug };
}

function renderSectionNode(node, context) {
  const heading = registerHeading(node.heading, context);
  context.sectionIds.add(heading.id);
  const headingHtml = `<h${heading.level} id="${escapeAttribute(heading.id)}">${renderInline(heading.children, context)}</h${heading.level}>`;
  const meta = context.sectionBindings[heading.id];
  let body;
  let classAttribute = "";
  if (meta !== undefined) {
    if (!meta.shapeValid || !SECTION_COMPONENTS.has(meta.component)) {
      const warning = sectionWarning(meta);
      meta.record.warning = warning;
      context.warnings.push(warning);
      body = renderSectionBody(node.body, context);
    } else {
      const component = renderSectionComponent(meta.component, node.body, context);
      if (component === null) {
        const warning = sectionWarning(meta);
        meta.record.warning = warning;
        context.warnings.push(warning);
        body = renderSectionBody(node.body, context);
      } else {
        meta.record.valid = true;
        meta.record.sectionClass = meta.classValue;
        classAttribute = meta.classValue === null ? "" : ` class="${escapeAttribute(meta.classValue)}"`;
        body = component;
      }
    }
  } else {
    body = renderSectionBody(node.body, context);
  }
  return `<section data-md-section="${escapeAttribute(heading.id)}"${classAttribute}>${headingHtml}${body}</section>`;
}

function renderGroupedBlocks(nodes, context) {
  return (nodes ?? []).map((node) => renderSectionBodyNode(node, context)).join("");
}

function finishSectionBindings(context) {
  for (const meta of context.bindingRecords) {
    if (context.sectionIds.has(meta.slug)) continue;
    const record = meta.record;
    delete record.class;
    delete record.sectionClass;
    delete record.warning;
    record.valid = false;
    record.error = { code: "E-SECT-01", target: meta.slug };
    record.runtimeTarget = null;
    context.errors.push(record.error);
  }
}

function renderFootnotes(context) {
  const items = [];
  for (let i = 0; i < context.footnoteOrder.length; i++) {
    const id = context.footnoteOrder[i];
    if (!own(context.footnotes, id)) continue;
    items.push(`<li id="fn-${escapeAttribute(id)}">${renderBlocks(context.footnotes[id], context)}</li>`);
  }
  return items.length === 0
    ? ""
    : `<section class="footnotes" data-md-footnotes><ol>${items.join("")}</ol></section>`;
}

export function renderDocument(parsed, options) {
  const context = {
    usedIds: new Set(),
    headings: [],
    warnings: [],
    references: parsed.references ?? null,
    footnotes: parsed.footnotes ?? Object.create(null),
    footnoteOrder: [],
    footnoteSeen: new Set(),
  };
  if (options?.sections === undefined) {
    const html = renderBlocks(parsed.blocks, context) + renderFootnotes(context);
    return { html, headings: context.headings, warnings: context.warnings };
  }
  context.sectionIds = new Set();
  context.errors = [];
  prepareSectionBindings(options.sections, context);
  const html = renderGroupedBlocks(groupSections(parsed.blocks), context) + renderFootnotes(context);
  finishSectionBindings(context);
  return {
    html,
    headings: context.headings,
    warnings: context.warnings,
    bindings: context.bindingRecords.map((meta) => meta.record),
    errors: context.errors,
  };
}

export function renderMarkdown(source, options) {
  return renderDocument(parseMarkdown(source), options);
}
