// Block-level Markdown parser for the mdhtml 1.0 subset (SPEC.md §9, PARSE-02).
// Dependency-free. Produces a data-only AST; HTML rendering and escaping belong
// to later checkpoints. Malformed or ambiguous input degrades to text and is
// never dropped.

const isBlank = (line) => line.trim() === "";

function splitLines(source) {
  const lines = [];
  let start = 0;
  for (let i = 0; i <= source.length; i++) {
    if (i === source.length || source.charCodeAt(i) === 10) {
      let text = source.slice(start, i);
      if (text.endsWith("\r")) text = text.slice(0, -1);
      lines.push(text);
      start = i + 1;
    }
  }
  return lines;
}

function leadingSpaces(line) {
  let n = 0;
  while (n < line.length && line[n] === " ") n++;
  return n;
}

function dedent(line, max) {
  let n = 0;
  while (n < line.length && n < max && line[n] === " ") n++;
  return line.slice(n);
}

const FENCE_RE = /^ {0,3}(`{3,}|~{3,})(.*)$/;
const CLOSE_FENCE_RE = /^ {0,3}(`{3,}|~{3,})[ \t]*$/;
const CONTAINER_OPEN_RE =
  /^ {0,3}:{3,}[ \t]+(?:([a-z][a-z0-9-]*)|\{\.([a-z][a-z0-9-]*)\})(?:[ \t]*\|[ \t]*(.*))?$/;
const CONTAINER_CLOSE_RE = /^[ \t]*:{3,}[ \t]*$/;

const stripLeading = (line) => line.replace(/^ +/, "");

function matchFence(line) {
  const m = FENCE_RE.exec(line);
  if (!m) return null;
  if (m[1][0] === "`" && m[2].includes("`")) return null;
  const info = m[2].trim();
  const language = info === "" ? null : info.split(/[ \t]+/)[0];
  return { char: m[1][0], length: m[1].length, indent: leadingSpaces(line), language };
}

function matchContainerOpener(line) {
  const match = CONTAINER_OPEN_RE.exec(line);
  if (match === null) return null;
  const argument = match[3] === undefined ? null : match[3].trim() || null;
  return { name: match[1] ?? match[2], argument };
}

function isContainerClose(line) {
  return CONTAINER_CLOSE_RE.test(line);
}

function stripQuotePrefixes(line) {
  let rest = line;
  for (let count = 0; count < 16; count++) {
    const quoted = matchQuote(rest);
    if (quoted === null) break;
    rest = quoted;
  }
  return rest;
}

function stripStructuralPrefixes(line) {
  let rest = line;
  let listIndent = 0;
  for (let count = 0; count < 16; count++) {
    const quoted = matchQuote(rest);
    if (quoted !== null) {
      rest = quoted;
      continue;
    }
    const marker = listMarkerAt(rest);
    if (marker === null || marker.content === "") break;
    rest = rest.slice(marker.contentIndent);
    listIndent += marker.contentIndent;
  }
  return { line: rest, listIndent };
}

function findPrefixedFenceClose(lines, start, fence, listIndent) {
  for (let i = start + 1; i < lines.length; i++) {
    const line = stripQuotePrefixes(lines[i]);
    if (listIndent > 0 && leadingSpaces(line) < listIndent) continue;
    const content = dedent(line, listIndent);
    const match = CLOSE_FENCE_RE.exec(content);
    if (match && match[1][0] === fence.char && match[1].length >= fence.length) return i;
  }
  return -1;
}

function findContainerClose(lines, start) {
  let nested = 0;
  for (let i = start + 1; i < lines.length; i++) {
    const structural = stripStructuralPrefixes(lines[i]);
    const fence = matchFence(structural.line);
    if (fence) {
      const close = findPrefixedFenceClose(lines, i, fence, structural.listIndent);
      if (close < 0) return -1;
      i = close;
      continue;
    }
    if (matchContainerOpener(structural.line) !== null) {
      nested++;
      continue;
    }
    if (!isContainerClose(structural.line)) continue;
    if (nested === 0) return i;
    nested--;
  }
  return -1;
}

function consumeFence(lines, start, fence) {
  const valueLines = [];
  let i = start + 1;
  let closed = false;
  while (i < lines.length) {
    const line = lines[i];
    const m = CLOSE_FENCE_RE.exec(line);
    if (m && m[1][0] === fence.char && m[1].length >= fence.length && leadingSpaces(line) <= fence.indent) {
      closed = true;
      break;
    }
    valueLines.push(dedent(line, fence.indent));
    i++;
  }
  if (!closed && valueLines.length > 0 && valueLines[valueLines.length - 1] === "") {
    valueLines.pop();
  }
  const endsWithNewline = closed || lines[lines.length - 1] === "";
  const value = valueLines.length === 0 ? "" : valueLines.join("\n") + (endsWithNewline ? "\n" : "");
  return { value, consumed: closed ? i - start + 1 : i - start, closed };
}

const ATX_RE = /^ {0,3}(#{1,6})(?:[ \t]+|$)/;

function matchHeading(line) {
  const m = ATX_RE.exec(line);
  if (!m) return null;
  let text = line.slice(m[0].length);
  text = text.replace(/^[ \t]+/, "");
  text = text.replace(/[ \t]+$/, "");
  text = text.replace(/[ \t]+#+$/, "");
  return { level: m[1].length, text };
}

const HR_RE = /^ {0,3}(?:(?:\*[ \t]*){3,}|(?:-[ \t]*){3,}|(?:_[ \t]*){3,})$/;

const QUOTE_RE = /^ {0,3}> ?/;

function matchQuote(line) {
  const m = QUOTE_RE.exec(line);
  return m === null ? null : line.slice(m[0].length);
}

const BULLET_MARKER_RE = /^([-+*])(?:[ \t]+|$)/;
const ORDERED_MARKER_RE = /^(\d{1,9}[.)])(?:[ \t]+|$)/;

function listMarkerAt(line) {
  const indent = leadingSpaces(line);
  if (indent > 3) return null;
  const rest = line.slice(indent);
  let m = BULLET_MARKER_RE.exec(rest);
  let ordered = false;
  let type;
  let start = 1;
  let width;
  let spaces = 0;
  if (m !== null) {
    type = m[1];
    width = 1;
    spaces = m[0].length - 1;
  } else {
    m = ORDERED_MARKER_RE.exec(rest);
    if (m === null) return null;
    type = m[1].slice(-1);
    start = parseInt(m[1].slice(0, -1), 10);
    ordered = true;
    width = m[1].length;
    spaces = m[0].length - width;
  }
  const content = line.slice(indent + width + Math.min(spaces, 4));
  const contentIndent = content === "" ? indent + width + 1 : indent + width + Math.min(spaces, 4);
  return { ordered, type, start, indent, contentIndent, content };
}

const TASK_RE = /^\[([ xX])\](?:[ \t]+|$)/;

function startsStructuralBlock(line) {
  if (matchFence(line)) return true;
  if (matchContainerOpener(line)) return true;
  if (matchHeading(line)) return true;
  if (HR_RE.test(line)) return true;
  if (matchQuote(line) !== null) return true;
  if (listMarkerAt(line)) return true;
  return false;
}

function interruptsParagraph(line, next, lines, index) {
  if (matchFence(line)) return true;
  if (matchContainerOpener(line)) return findContainerClose(lines, index) >= 0;
  if (matchHeading(line)) return true;
  if (HR_RE.test(line)) return true;
  if (matchQuote(line) !== null) return true;
  const marker = listMarkerAt(line);
  if (marker) {
    if (marker.content === "") return false;
    if (marker.ordered && marker.start !== 1) return false;
    return true;
  }
  if (next !== undefined && hasUnescapedPipe(line) && isDelimiterRow(next)) return true;
  return false;
}

function consumeParagraph(lines, start) {
  const text = [];
  let i = start;
  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) break;
    if (i > start && interruptsParagraph(line, lines[i + 1], lines, i)) break;
    // First-line indentation is structural and removed; leading whitespace on
    // continuation lines is paragraph payload and is preserved verbatim.
    text.push(i === start ? stripLeading(line) : line);
    i++;
  }
  return { block: { type: "paragraph", text: text.join("\n") }, next: i };
}

function consumeQuote(lines, start, references, footnotes) {
  const content = [];
  let i = start;
  let openParagraph = false;
  while (i < lines.length) {
    const line = lines[i];
    const quoted = matchQuote(line);
    if (quoted !== null) {
      content.push(quoted);
      openParagraph = quoted !== "" && !startsStructuralBlock(quoted);
      i++;
      continue;
    }
    if (isBlank(line)) break;
    if (openParagraph && !startsStructuralBlock(line)) {
      content.push(line);
      i++;
      continue;
    }
    break;
  }
  while (content.length > 0 && content[content.length - 1] === "") content.pop();
  return { block: { type: "blockquote", children: parseBlocks(content, references, footnotes) }, next: i };
}

function consumeListItem(lines, start, references, footnotes) {
  const marker = listMarkerAt(lines[start]);
  const contentIndent = marker.contentIndent;
  const contentLines = [];
  let i = start + 1;
  let openParagraph = false;
  const firstContent = marker.content;
  if (firstContent !== "") {
    contentLines.push(firstContent);
    openParagraph = !startsStructuralBlock(firstContent);
  }
  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) {
      let j = i + 1;
      while (j < lines.length && isBlank(lines[j])) j++;
      if (j >= lines.length) break;
      const nextMarker = listMarkerAt(lines[j]);
      if (leadingSpaces(lines[j]) >= contentIndent) {
        contentLines.push("");
        openParagraph = false;
        i++;
        continue;
      }
      if (nextMarker !== null && nextMarker.type === marker.type) {
        i = j;
        break;
      }
      break;
    }
    if (leadingSpaces(line) >= contentIndent) {
      const content = dedent(line, contentIndent);
      contentLines.push(content);
      openParagraph = content !== "" && !startsStructuralBlock(content);
      i++;
      continue;
    }
    if (listMarkerAt(line)) break;
    if (openParagraph && !startsStructuralBlock(line)) {
      contentLines.push(line);
      i++;
      continue;
    }
    break;
  }
  while (contentLines.length > 0 && contentLines[contentLines.length - 1] === "") contentLines.pop();

  let checked = null;
  if (contentLines.length > 0) {
    const task = TASK_RE.exec(contentLines[0]);
    if (task) {
      checked = task[1] === "x" || task[1] === "X";
      contentLines[0] = contentLines[0].slice(task[0].length);
    }
  }
  return {
    block: { checked, children: parseBlocks(contentLines, references, footnotes) },
    next: i,
  };
}

function consumeList(lines, start, references, footnotes) {
  const first = listMarkerAt(lines[start]);
  const items = [];
  let i = start;
  while (i < lines.length) {
    const marker = listMarkerAt(lines[i]);
    if (marker === null || marker.type !== first.type) break;
    const item = consumeListItem(lines, i, references, footnotes);
    items.push(item.block);
    i = item.next;
  }
  return {
    block: { type: "list", ordered: first.ordered, start: first.ordered ? first.start : 1, items },
    next: i,
  };
}

const DELIM_CELL_RE = /^:?-+:?$/;
const DELIM_ROW_RE = /^ {0,3}\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?$/;

function hasUnescapedPipe(line) {
  let escaped = false;
  for (const ch of line) {
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === "\\") {
      escaped = true;
      continue;
    }
    if (ch === "|") return true;
  }
  return false;
}

function splitTableRow(line) {
  const cells = [];
  let current = "";
  let escaped = false;
  for (const ch of line) {
    if (escaped) {
      current += ch;
      escaped = false;
      continue;
    }
    if (ch === "\\") {
      escaped = true;
      current += ch;
      continue;
    }
    if (ch === "|") {
      cells.push(current);
      current = "";
      continue;
    }
    current += ch;
  }
  cells.push(current);
  if (cells.length > 1 && cells[0] === "") cells.shift();
  if (cells.length > 1 && cells[cells.length - 1] === "") cells.pop();
  return cells.map((cell) => cell.trim());
}

function isDelimiterRow(line) {
  if (!DELIM_ROW_RE.test(line)) return false;
  const cells = splitTableRow(line);
  return cells.length > 0 && cells.every((cell) => DELIM_CELL_RE.test(cell));
}

function isTableStart(lines, i) {
  const header = lines[i];
  const delimiter = lines[i + 1];
  if (delimiter === undefined) return false;
  if (!hasUnescapedPipe(header)) return false;
  if (!isDelimiterRow(delimiter)) return false;
  return splitTableRow(header).length === splitTableRow(delimiter).length;
}

function tableAlign(cell) {
  if (cell.startsWith(":") && cell.endsWith(":")) return "center";
  if (cell.startsWith(":")) return "left";
  if (cell.endsWith(":")) return "right";
  return null;
}

function normalizeCells(cells, count) {
  const out = cells.slice(0, count);
  while (out.length < count) out.push("");
  return out;
}

function consumeTable(lines, start) {
  const delimiterCells = splitTableRow(lines[start + 1]);
  const count = delimiterCells.length;
  const align = delimiterCells.map(tableAlign);
  const headerCells = splitTableRow(lines[start]);
  const header = normalizeCells(headerCells, count);
  const rows = [];
  const sourceCellCounts = { header: headerCells.length, rows: [] };
  let i = start + 2;
  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) break;
    if (startsStructuralBlock(line)) break;
    const cells = splitTableRow(line);
    sourceCellCounts.rows.push(cells.length);
    rows.push(normalizeCells(cells, count));
    i++;
  }
  return { block: { type: "table", align, header, rows, sourceCellCounts }, next: i };
}

const FOOTNOTE_DEF_RE = /^ {0,3}\[\^([^\]\n]+)\]:[ \t]*(.*)$/;
const REF_DEF_RE =
  /^ {0,3}\[([^\]\n]+)\]:[ \t]*(?:<([^>\n]+)>|([^ \t\n]+))(?:(?:[ \t]+)(?:"((?:[^"\\\n]|\\.)*)"|'((?:[^'\\\n]|\\.)*)'|\(((?:[^()\\\n]|\\.)*)\)))?[ \t]*$/;

function normalizeLabel(label) {
  return label.trim().replace(/\s+/g, " ").toLowerCase();
}

function consumeDefinition(lines, i, references, footnotes) {
  const line = lines[i];
  const footnote = FOOTNOTE_DEF_RE.exec(line);
  if (footnote) {
    const id = footnote[1];
    const content = [footnote[2]];
    let j = i + 1;
    while (j < lines.length) {
      const next = lines[j];
      if (isBlank(next)) {
        let k = j + 1;
        while (k < lines.length && isBlank(lines[k])) k++;
        if (k < lines.length && leadingSpaces(lines[k]) >= 4) {
          content.push("");
          j++;
          continue;
        }
        break;
      }
      if (leadingSpaces(next) >= 4) {
        content.push(dedent(next, 4));
        j++;
        continue;
      }
      break;
    }
    while (content.length > 0 && content[content.length - 1] === "") content.pop();
    if (!(id in footnotes)) {
      footnotes[id] = parseBlocks(content, references, footnotes);
    }
    return j - i;
  }
  const reference = REF_DEF_RE.exec(line);
  if (reference) {
    const key = normalizeLabel(reference[1]);
    if (!(key in references)) {
      references[key] = {
        url: reference[2] ?? reference[3],
        title: reference[4] ?? reference[5] ?? reference[6] ?? null,
      };
    }
    return 1;
  }
  return 0;
}

function parseBlocks(lines, references, footnotes) {
  const blocks = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) {
      i++;
      continue;
    }
    const consumed = consumeDefinition(lines, i, references, footnotes);
    if (consumed > 0) {
      i += consumed;
      continue;
    }
    const container = matchContainerOpener(line);
    if (container) {
      const close = findContainerClose(lines, i);
      if (close >= 0) {
        blocks.push({
          type: "container",
          name: container.name,
          argument: container.argument,
          children: parseBlocks(lines.slice(i + 1, close), references, footnotes),
        });
        i = close + 1;
        continue;
      }
    }
    const fence = matchFence(line);
    if (fence) {
      const result = consumeFence(lines, i, fence);
      blocks.push({ type: "codeBlock", language: fence.language, value: result.value });
      i += result.consumed;
      continue;
    }
    const heading = matchHeading(line);
    if (heading) {
      blocks.push({ type: "heading", level: heading.level, text: heading.text });
      i++;
      continue;
    }
    if (HR_RE.test(line)) {
      blocks.push({ type: "thematicBreak" });
      i++;
      continue;
    }
    if (matchQuote(line) !== null) {
      const result = consumeQuote(lines, i, references, footnotes);
      blocks.push(result.block);
      i = result.next;
      continue;
    }
    if (listMarkerAt(line)) {
      const result = consumeList(lines, i, references, footnotes);
      blocks.push(result.block);
      i = result.next;
      continue;
    }
    if (isTableStart(lines, i)) {
      const result = consumeTable(lines, i);
      blocks.push(result.block);
      i = result.next;
      continue;
    }
    const paragraph = consumeParagraph(lines, i);
    blocks.push(paragraph.block);
    i = paragraph.next;
  }
  return blocks;
}

export function parseMarkdownBlocks(source) {
  const references = Object.create(null);
  const footnotes = Object.create(null);
  const blocks = parseBlocks(splitLines(source), references, footnotes);
  return { blocks, references, footnotes };
}

const ASCII_PUNCTUATION = new Set("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~");
const MAX_INLINE_DEPTH = 64;

function decodeEscapes(value) {
  let result = "";
  for (let i = 0; i < value.length; i++) {
    if (value[i] === "\\" && i + 1 < value.length && ASCII_PUNCTUATION.has(value[i + 1])) {
      result += value[++i];
    } else {
      result += value[i];
    }
  }
  return result;
}

function appendText(nodes, value) {
  if (value === "") return;
  const previous = nodes[nodes.length - 1];
  if (previous?.type === "text") {
    previous.value += value;
  } else {
    nodes.push({ type: "text", value });
  }
}

function appendDecodedText(nodes, value) {
  appendText(nodes, decodeEscapes(value));
}

function isEscaped(text, index) {
  let backslashes = 0;
  for (let i = index - 1; i >= 0 && text[i] === "\\"; i--) backslashes++;
  return backslashes % 2 === 1;
}

function codeSpanSegments(text) {
  const segments = [];
  let plainStart = 0;
  let i = 0;
  while (i < text.length) {
    if (text[i] !== "`" || isEscaped(text, i)) {
      i++;
      continue;
    }
    let run = 1;
    while (i + run < text.length && text[i + run] === "`") run++;
    let close = i + run;
    let closeStart = -1;
    while (close < text.length) {
      if (text[close] !== "`") {
        close++;
        continue;
      }
      let closeRun = 1;
      while (close + closeRun < text.length && text[close + closeRun] === "`") closeRun++;
      if (closeRun === run) {
        closeStart = close;
        break;
      }
      close += closeRun;
    }
    if (closeStart < 0) {
      i += run;
      continue;
    }
    if (plainStart < i) segments.push({ type: "text", value: text.slice(plainStart, i) });
    segments.push({ type: "code", value: text.slice(i + run, closeStart) });
    i = closeStart + run;
    plainStart = i;
  }
  if (plainStart < text.length) segments.push({ type: "text", value: text.slice(plainStart) });
  return segments;
}

function hardBreakOrNewline(nodes, buffer) {
  const match = /( +)$/.exec(buffer);
  if (match === null || match[1].length < 2) {
    appendDecodedText(nodes, `${buffer}\n`);
    return "";
  }
  appendDecodedText(nodes, buffer.slice(0, -2));
  nodes.push({ type: "hardBreak" });
  return "";
}

function findBracketClose(text, start) {
  let depth = 0;
  for (let i = start; i < text.length; i++) {
    if (text[i] === "\\") {
      i++;
      continue;
    }
    if (text[i] === "[") {
      depth++;
    } else if (text[i] === "]") {
      if (depth === 0) return i;
      depth--;
    }
  }
  return -1;
}

function findParenthesisClose(text, start) {
  let depth = 0;
  for (let i = start; i < text.length; i++) {
    if (text[i] === "\\") {
      i++;
      continue;
    }
    if (text[i] === "(") depth++;
    if (text[i] === ")") {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function parseInlineDestination(text, start) {
  if (text[start] !== "(") return null;
  const close = findParenthesisClose(text, start);
  if (close < 0) return { literalEnd: text.length };
  const content = text.slice(start + 1, close).trim();
  const match = /^([^\s]+)(?:\s+(.+))?$/.exec(content);
  if (!match) return { literalEnd: close + 1 };
  let title = null;
  if (match[2] !== undefined) {
    const titleMatch = /^"((?:[^"\\]|\\.)*)"$/.exec(match[2]);
    if (!titleMatch) return { literalEnd: close + 1 };
    title = decodeEscapes(titleMatch[1]);
  }
  return {
    end: close + 1,
    url: decodeEscapes(match[1]),
    title,
  };
}

function parseBracket(text, start, references, depth) {
  const image = text[start] === "!";
  const labelStart = start + (image ? 2 : 1);
  if (image && text[start + 1] !== "[") return null;
  const labelEnd = findBracketClose(text, labelStart);
  if (labelEnd < 0) return null;
  const label = text.slice(labelStart, labelEnd);
  if (!image && label.startsWith("^") && label.length > 1) {
    return {
      end: labelEnd + 1,
      node: { type: "footnoteReference", id: label.slice(1) },
    };
  }
  const afterLabel = labelEnd + 1;
  const inline = parseInlineDestination(text, afterLabel);
  if (inline?.end !== undefined) {
    if (image) {
      return {
        end: inline.end,
        node: {
          type: "image",
          src: inline.url,
          title: inline.title,
          alt: decodeEscapes(label),
        },
      };
    }
    return {
      end: inline.end,
      node: {
        type: "link",
        url: inline.url,
        title: inline.title,
        children: parseInlineAtDepth(label, references, depth + 1),
      },
    };
  }
  if (inline?.literalEnd !== undefined) return { literalEnd: inline.literalEnd };

  if (text[afterLabel] === "[") {
    const refEnd = findBracketClose(text, afterLabel + 1);
    if (refEnd < 0) return { literalEnd: text.length };
    const id = text.slice(afterLabel + 1, refEnd);
    const key = normalizeLabel(id);
    if (id !== "" && references !== null && Object.prototype.hasOwnProperty.call(references, key)) {
      const reference = references[key];
      if (image) {
        return {
          end: refEnd + 1,
          node: { type: "image", src: reference.url, title: reference.title, alt: decodeEscapes(label) },
        };
      }
      return {
        end: refEnd + 1,
        node: {
          type: "link",
          url: reference.url,
          title: reference.title,
          children: parseInlineAtDepth(label, references, depth + 1),
        },
      };
    }
    return { literalEnd: refEnd + 1 };
  }
  return null;
}

function delimiterRun(text, start, delimiter) {
  let count = 0;
  while (text[start + count] === delimiter) count++;
  return count;
}

function findClosingDelimiter(text, start, delimiter, length, exact) {
  for (let i = start; i < text.length; i++) {
    if (text[i] === "\\") {
      i++;
      continue;
    }
    if (text[i] !== delimiter) continue;
    const run = delimiterRun(text, i, delimiter);
    if ((exact && run !== length) || (!exact && run < length)) {
      i += run - 1;
      continue;
    }
    const close = exact ? i : i + run - length;
    if (close > start) return close;
    i += run - 1;
  }
  return -1;
}

function parseTextSegment(text, references, depth) {
  const nodes = [];
  let buffer = "";
  let i = 0;
  const flush = () => {
    appendDecodedText(nodes, buffer);
    buffer = "";
  };
  if (depth >= MAX_INLINE_DEPTH) {
    for (const ch of text) {
      if (ch === "\n") buffer = hardBreakOrNewline(nodes, buffer);
      else buffer += ch;
    }
    flush();
    return nodes;
  }
  while (i < text.length) {
    const ch = text[i];
    if (ch === "\n") {
      buffer = hardBreakOrNewline(nodes, buffer);
      i++;
      continue;
    }
    if (ch === "\\" && i + 1 < text.length && ASCII_PUNCTUATION.has(text[i + 1])) {
      buffer += text.slice(i, i + 2);
      i += 2;
      continue;
    }
    if (ch === "[" || (ch === "!" && text[i + 1] === "[")) {
      const bracket = parseBracket(text, i, references, depth);
      if (bracket?.node !== undefined) {
        flush();
        nodes.push(bracket.node);
        i = bracket.end;
        continue;
      }
      if (bracket?.literalEnd !== undefined) {
        buffer += text.slice(i, bracket.literalEnd);
        i = bracket.literalEnd;
        continue;
      }
    }
    let kind = null;
    let length = 0;
    let exact = false;
    if (text.startsWith("~~", i)) {
      kind = "strike";
      length = 2;
    } else if (text.startsWith("**", i) || text.startsWith("__", i)) {
      kind = "strong";
      length = 2;
    } else if ((ch === "*" || ch === "_") && delimiterRun(text, i, ch) === 1) {
      kind = "emphasis";
      length = 1;
      exact = true;
    }
    if (kind !== null) {
      const close = findClosingDelimiter(text, i + length, ch, length, exact);
      if (close > i + length) {
        flush();
        nodes.push({
          type: kind,
          children: parseInlineAtDepth(text.slice(i + length, close), references, depth + 1),
        });
        i = close + length;
        continue;
      }
      const run = delimiterRun(text, i, ch);
      buffer += text.slice(i, i + Math.max(run, length));
      i += Math.max(run, length);
      continue;
    }
    buffer += ch;
    i++;
  }
  flush();
  return nodes;
}

function parseInlineAtDepth(text, references, depth) {
  if (typeof text !== "string" || text === "") return [];
  const nodes = [];
  for (const segment of codeSpanSegments(text)) {
    if (segment.type === "code") nodes.push({ type: "code", value: segment.value });
    else nodes.push(...parseTextSegment(segment.value, references, depth));
  }
  return nodes;
}

export function parseInline(text, references = null) {
  return parseInlineAtDepth(text, references, 0);
}

function enrichBlock(block, references) {
  if (block.type === "paragraph" || block.type === "heading") {
    return { ...block, children: parseInline(block.text, references) };
  }
  if (block.type === "blockquote") {
    return { ...block, children: block.children.map((child) => enrichBlock(child, references)) };
  }
  if (block.type === "container") {
    return { ...block, children: block.children.map((child) => enrichBlock(child, references)) };
  }
  if (block.type === "list") {
    return {
      ...block,
      items: block.items.map((item) => ({
        ...item,
        children: item.children.map((child) => enrichBlock(child, references)),
      })),
    };
  }
  if (block.type === "table") {
    return {
      ...block,
      headerInlines: block.header.map((cell) => parseInline(cell, references)),
      rowInlines: block.rows.map((row) => row.map((cell) => parseInline(cell, references))),
    };
  }
  return block;
}

function enrichFootnotes(footnotes, references) {
  const result = Object.create(null);
  for (const [id, blocks] of Object.entries(footnotes)) {
    result[id] = blocks.map((block) => enrichBlock(block, references));
  }
  return result;
}

export function parseMarkdown(source) {
  const parsed = parseMarkdownBlocks(source);
  return {
    ...parsed,
    blocks: parsed.blocks.map((block) => enrichBlock(block, parsed.references)),
    footnotes: enrichFootnotes(parsed.footnotes, parsed.references),
  };
}
