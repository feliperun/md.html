// Strict parser for the mdhtml 1.0 front matter subset (SPEC.md §8, PARSE-01).
// Dependency-free; the Markdown body is preserved byte for byte.

const isSpace = (c) => c === " " || c === "\t";

export class FrontMatterError extends Error {
  constructor(message, line, column, code = "E-PARSE-01") {
    super(message);
    this.name = "FrontMatterError";
    this.code = code;
    this.line = line;
    this.column = column;
  }
}

function splitLines(source) {
  const lines = [];
  let start = 0;
  for (let i = 0; i <= source.length; i++) {
    if (i === source.length || source.charCodeAt(i) === 10) {
      let text = source.slice(start, i);
      if (text.endsWith("\r")) text = text.slice(0, -1);
      lines.push({ index: lines.length, start, end: i, text });
      start = i + 1;
    }
  }
  return lines;
}

// -1 blank, -2 tab in indentation position, >= 0 first non-space column.
function indentOf(text) {
  for (let i = 0; i < text.length; i++) {
    if (text[i] === " ") continue;
    if (text[i] === "\t") return -2;
    return i;
  }
  return -1;
}

function resolveScalar(raw, line, column) {
  if (raw === "null") return null;
  if (raw === "true") return true;
  if (raw === "false") return false;
  const numeric = /^-?[0-9]+$/.test(raw)
    || /^-?(?:[0-9]+\.[0-9]*|[0-9]*\.[0-9]+)(?:[eE][+-]?[0-9]+)?$/.test(raw)
    || /^-?[0-9]+[eE][+-]?[0-9]+$/.test(raw);
  if (numeric) {
    const value = Number(raw);
    if (!Number.isFinite(value)) throw new FrontMatterError("numeric scalar must be finite", line, column);
    return value;
  }
  return raw;
}

function canonicalKey(key) {
  return typeof key === "string" ? key : String(key);
}

const ORDERED_KEYS = Symbol("front matter ordered keys");

function finalizeMap(order) {
  const map = Object.fromEntries(order.map(([key, value]) => [String(key), value]));
  Object.defineProperty(map, ORDERED_KEYS, {
    value: order.map(([key]) => String(key)),
    enumerable: false,
  });
  return map;
}

function isSeqIndicator(text, col) {
  return text[col] === "-" && (col + 1 >= text.length || text[col + 1] === " ");
}

class Parser {
  constructor(source, lines) {
    this.source = source;
    this.lines = lines;
    this.idx = 1; // skip the opening delimiter line
  }

  error(message, line, column) {
    return new FrontMatterError(message, line, column);
  }

  peek() {
    let i = this.idx;
    while (i < this.lines.length) {
      const line = this.lines[i];
      const col = indentOf(line.text);
      if (col === -1) {
        i++;
        continue;
      }
      if (col === -2) {
        const tab = line.text.indexOf("\t");
        throw this.error("tab indentation is not allowed", line.index + 1, tab + 1);
      }
      if (line.text[col] === "#") {
        i++;
        continue;
      }
      return { line, col, index: i };
    }
    return null;
  }

  parse() {
    const fm = this.parseBlock(0, true);
    const sig = this.peek();
    if (!sig || sig.line.text !== "---") {
      const atLine = sig ? sig.line.index + 1 : this.lines.length;
      throw this.error("unterminated front matter", atLine, 1);
    }
    this.idx = sig.index + 1;
    const bodyStart = sig.line.end >= this.source.length ? this.source.length : sig.line.end + 1;
    return {
      frontMatter: fm === null ? {} : fm,
      body: this.source.slice(bodyStart),
      raw: this.source.slice(0, bodyStart),
      bodyOffset: bodyStart,
    };
  }

  parseBlock(indent, isTop = false) {
    const sig = this.peek();
    if (!sig) return null;
    if (sig.line.text === "---" && sig.col === 0) return null;
    if (sig.line.text === "..." && sig.col === 0) {
      throw this.error("document end marker is not allowed", sig.line.index + 1, 1);
    }
    if (sig.col < indent) return null;
    if (sig.col > indent) {
      throw this.error("inconsistent indentation", sig.line.index + 1, sig.col + 1);
    }
    if (isSeqIndicator(sig.line.text, sig.col)) {
      if (isTop) {
        throw this.error("front matter must be a mapping", sig.line.index + 1, sig.col + 1);
      }
      return this.parseSeqBlock(indent);
    }
    return this.parseMapLoop(indent, [], new Map());
  }

  // Next line at exactly `indent`, or null to stop the caller's loop.
  // Shared by parseBlock's two loops, parseSeqBlock, and parseMapLoop.
  nextSibling(indent) {
    const s = this.peek();
    if (!s) return null;
    if (s.line.text === "---" && s.col === 0) return null;
    if (s.line.text === "..." && s.col === 0) {
      throw this.error("document end marker is not allowed", s.line.index + 1, 1);
    }
    if (s.col < indent) return null;
    if (s.col > indent) {
      throw this.error("inconsistent indentation", s.line.index + 1, s.col + 1);
    }
    return s;
  }

  parseSeqBlock(indent) {
    const items = [];
    while (true) {
      const s = this.nextSibling(indent);
      if (!s) break;
      if (!isSeqIndicator(s.line.text, s.col)) {
        throw this.error("cannot mix mappings and sequences at the same indentation", s.line.index + 1, s.col + 1);
      }
      this.idx = s.index + 1;
      items.push(this.parseSeqItem(s.line, indent));
    }
    return items;
  }

  parseMapLoop(indent, order, seen) {
    while (true) {
      const s = this.nextSibling(indent);
      if (!s) break;
      if (isSeqIndicator(s.line.text, s.col)) {
        throw this.error("cannot mix mappings and sequences at the same indentation", s.line.index + 1, s.col + 1);
      }
      this.idx = s.index + 1;
      const [key, value] = this.parseMapEntryAt(indent, s.index);
      const canon = canonicalKey(key);
      if (seen.has(canon)) {
        throw this.error(`duplicate key '${String(key)}'`, s.line.index + 1, s.col + 1);
      }
      seen.set(canon, true);
      order.push([key, value]);
    }
    return finalizeMap(order);
  }

  parseMapEntryAt(indent, index) {
    const line = this.lines[index];
    const text = line.text;
    let key;
    let colon;
    if (text[indent] === "'" || text[indent] === '"') {
      const quoted = this.scanQuoted(text, indent, line);
      key = quoted.value;
      colon = quoted.end;
      if (text[colon] !== ":") {
        throw this.error("expected ':' after quoted key", line.index + 1, colon + 1);
      }
      if (colon + 1 < text.length && !isSpace(text[colon + 1])) {
        throw this.error("expected space after ':'", line.index + 1, colon + 2);
      }
    } else {
      colon = this.scanPlainKey(text, indent, line);
      const rawKey = text.slice(indent, colon);
      this.assertPlainStart(rawKey, line.index + 1, indent + 1);
      key = resolveScalar(rawKey, line.index + 1, indent + 1);
    }
    const value = this.parseValueAfterColon(text, colon, indent, line);
    return [key, value];
  }

  scanPlainKey(text, start, line) {
    const colon = this.findPlainColon(text, start, line);
    if (colon < 0) {
      throw this.error("expected ':' after key", line.index + 1, start + 1);
    }
    return colon;
  }

  findPlainColon(text, start, line) {
    for (let i = start; i < text.length; i++) {
      const c = text[i];
      if (c === ":" && (i + 1 >= text.length || isSpace(text[i + 1]))) {
        if (i === start || isSpace(text[i - 1])) {
          throw this.error("unexpected space before ':'", line.index + 1, i + 1);
        }
        return i;
      }
      if (c === "#" && i > start && isSpace(text[i - 1])) {
        return -1;
      }
    }
    return -1;
  }

  assertPlainStart(raw, line, column) {
    const c = raw[0];
    if (c === "!") throw this.error("tags are not supported", line, column);
    if (c === "&") throw this.error("anchors are not supported", line, column);
    if (c === "*") throw this.error("aliases are not supported", line, column);
    if (c === "%" || c === "@" || c === "`") {
      throw this.error(`plain scalar cannot start with '${c}'`, line, column);
    }
  }

  parseValueAfterColon(text, colon, keyIndent, line) {
    let p = colon + 1;
    while (p < text.length && isSpace(text[p])) p++;
    if (p >= text.length) {
      const sig = this.peek();
      if (sig && sig.col > keyIndent) return this.parseBlock(sig.col);
      return null;
    }
    const ch = text[p];
    if (ch === "#") return null;
    if (ch === "|" || ch === ">") {
      const rest = text.slice(p + 1).trim();
      if (rest !== "" && !rest.startsWith("#")) {
        throw this.error("unexpected content after block scalar indicator", line.index + 1, p + 2);
      }
      return this.parseBlockScalar(keyIndent, ch, line);
    }
    if (ch === "[" || ch === "{") {
      const res = this.parseFlow(text, p, line);
      this.checkTrailing(text, res.end, line);
      return res.value;
    }
    if (ch === "'" || ch === '"') {
      const res = this.scanQuoted(text, p, line);
      this.checkTrailing(text, res.end, line);
      return res.value;
    }
    let end = text.length;
    for (let j = p; j < text.length; j++) {
      if (text[j] === "#" && j > p && isSpace(text[j - 1])) {
        end = j;
        break;
      }
    }
    const raw = text.slice(p, end).trim();
    if (raw === "") return null;
    this.assertPlainStart(raw, line.index + 1, p + 1);
    this.rejectMappingColon(raw, line.index + 1, p + 1);
    if (/^-\s/.test(raw)) {
      throw this.error("block sequence entry is not allowed as a value", line.index + 1, p + 1);
    }
    return resolveScalar(raw, line.index + 1, p + 1);
  }

  rejectMappingColon(raw, line, column) {
    for (let i = 0; i < raw.length; i++) {
      if (raw[i] === ":" && (i + 1 >= raw.length || isSpace(raw[i + 1]))) {
        throw this.error("mapping value not allowed in plain scalar", line, column + i);
      }
    }
  }

  parseSeqItem(line, indent) {
    const text = line.text;
    let p = indent + 1;
    while (p < text.length && text[p] === " ") p++;
    if (p >= text.length || text[p] === "#") return this.parseSeqItemEmpty(indent);
    const ch = text[p];
    if (ch === "-" && (p + 1 >= text.length || text[p + 1] === " ")) return this.parseNestedSeqItem(line, p);
    if (ch === "|" || ch === ">") return this.parseSeqItemBlockScalar(indent, ch, p, text, line);
    if (ch === "[" || ch === "{") return this.parseSeqItemFlow(text, p, line);
    if (ch === "'" || ch === '"') return this.parseSeqItemQuoted(text, p, line);
    const colon = this.findPlainColon(text, p, line);
    if (colon >= 0) return this.parseSeqItemMapEntry(text, p, colon, line);
    return this.parseSeqItemPlainScalar(text, p, line);
  }

  parseSeqItemEmpty(indent) {
    const sig = this.peek();
    if (sig && sig.col > indent) return this.parseBlock(sig.col);
    return null;
  }

  parseNestedSeqItem(line, p) {
    const items = [];
    items.push(this.parseSeqItem(line, p));
    while (true) {
      const s = this.nextSibling(p);
      if (!s) break;
      if (!isSeqIndicator(s.line.text, s.col)) {
        throw this.error("cannot mix mappings and sequences at the same indentation", s.line.index + 1, s.col + 1);
      }
      this.idx = s.index + 1;
      items.push(this.parseSeqItem(s.line, p));
    }
    return items;
  }

  parseSeqItemBlockScalar(indent, ch, p, text, line) {
    const rest = text.slice(p + 1).trim();
    if (rest !== "" && !rest.startsWith("#")) {
      throw this.error("unexpected content after block scalar indicator", line.index + 1, p + 2);
    }
    return this.parseBlockScalar(indent, ch, line);
  }

  parseSeqItemFlow(text, p, line) {
    const res = this.parseFlow(text, p, line);
    this.checkTrailing(text, res.end, line);
    return res.value;
  }

  parseSeqItemQuoted(text, p, line) {
    const res = this.scanQuoted(text, p, line);
    if (text[res.end] === ":") {
      if (res.end + 1 < text.length && !isSpace(text[res.end + 1])) {
        throw this.error("expected space after ':'", line.index + 1, res.end + 2);
      }
      const order = [[res.value, this.parseValueAfterColon(text, res.end, p, line)]];
      const seen = new Map([[canonicalKey(res.value), true]]);
      return this.parseMapLoop(p, order, seen);
    }
    this.checkTrailing(text, res.end, line);
    return res.value;
  }

  parseSeqItemMapEntry(text, p, colon, line) {
    const rawKey = text.slice(p, colon);
    this.assertPlainStart(rawKey, line.index + 1, p + 1);
    const key = resolveScalar(rawKey, line.index + 1, p + 1);
    const value = this.parseValueAfterColon(text, colon, p, line);
    const order = [[key, value]];
    const seen = new Map([[canonicalKey(key), true]]);
    return this.parseMapLoop(p, order, seen);
  }

  parseSeqItemPlainScalar(text, p, line) {
    let end = text.length;
    for (let j = p; j < text.length; j++) {
      if (text[j] === "#" && j > p && isSpace(text[j - 1])) {
        end = j;
        break;
      }
    }
    const raw = text.slice(p, end).trim();
    if (raw === "") return null;
    this.assertPlainStart(raw, line.index + 1, p + 1);
    return resolveScalar(raw, line.index + 1, p + 1);
  }

  parseBlockScalar(indent, ch, line) {
    const content = [];
    let blockIndent = null;
    while (this.idx < this.lines.length) {
      const l = this.lines[this.idx];
      const col = indentOf(l.text);
      if (col === -2) {
        const tab = l.text.indexOf("\t");
        throw this.error("tab indentation is not allowed", l.index + 1, tab + 1);
      }
      if (col === -1) {
        content.push("");
        this.idx++;
        continue;
      }
      if (col <= indent) break;
      if (blockIndent === null) blockIndent = col;
      if (col < blockIndent) {
        throw this.error("inconsistent indentation in block scalar", l.index + 1, col + 1);
      }
      content.push(l.text.slice(blockIndent));
      this.idx++;
    }
    if (ch === "|") {
      let value = content.join("\n");
      if (value === "") return "";
      value = value.replace(/\n+$/, "");
      return value + "\n";
    }
    return this.fold(content);
  }

  fold(content) {
    let out = "";
    let state = "start";
    for (const raw of content) {
      if (raw.trim() === "") {
        out += "\n";
        state = "blank";
        continue;
      }
      const extra = raw.length - raw.trimStart().length;
      const rest = raw.trimEnd();
      if (extra > 0) {
        out += "\n" + rest;
        state = "literal";
        continue;
      }
      if (state === "fold") out += " " + rest;
      else if (state === "literal") out += "\n" + rest;
      else out += rest;
      state = "fold";
    }
    if (out === "") return "";
    out = out.replace(/\n+$/, "");
    return out + "\n";
  }

  scanQuoted(text, start, line) {
    const quote = text[start];
    let i = start + 1;
    let out = "";
    while (i < text.length) {
      const c = text[i];
      if (quote === "'") {
        if (c === "'") {
          if (text[i + 1] === "'") {
            out += "'";
            i += 2;
            continue;
          }
          return { value: out, end: i + 1 };
        }
        out += c;
        i++;
        continue;
      }
      if (c === '"') return { value: out, end: i + 1 };
      if (c === "\\") {
        const e = text[i + 1];
        if (e === "u") {
          const hex = text.slice(i + 2, i + 6);
          if (!/^[0-9a-fA-F]{4}$/.test(hex)) {
            throw this.error("invalid unicode escape", line.index + 1, i + 1);
          }
          out += String.fromCharCode(parseInt(hex, 16));
          i += 6;
          continue;
        }
        switch (e) {
          case "\\": out += "\\"; break;
          case '"': out += '"'; break;
          case "n": out += "\n"; break;
          case "t": out += "\t"; break;
          case "r": out += "\r"; break;
          case "b": out += "\b"; break;
          case "f": out += "\f"; break;
          case "0": out += "\0"; break;
          default:
            throw this.error("invalid escape sequence", line.index + 1, i + 1);
        }
        i += 2;
        continue;
      }
      out += c;
      i++;
    }
    throw this.error("unterminated quoted scalar", line.index + 1, start + 1);
  }

  checkTrailing(text, end, line) {
    let i = end;
    let sawSep = false;
    while (i < text.length && isSpace(text[i])) {
      sawSep = true;
      i++;
    }
    if (i < text.length && (text[i] !== "#" || !sawSep)) {
      throw this.error("unexpected content after value", line.index + 1, i + 1);
    }
  }

  parseFlow(text, start, line) {
    const open = text[start];
    const close = open === "[" ? "]" : "}";
    const isMap = open === "{";
    const values = [];
    const order = [];
    const seen = new Map();
    let i = start + 1;
    while (true) {
      while (i < text.length && text[i] === " ") i++;
      if (i >= text.length) throw this.error("unterminated flow collection", line.index + 1, start + 1);
      if (text[i] === close) {
        i++;
        break;
      }
      if (text[i] === ",") {
        throw this.error("empty element in flow collection", line.index + 1, i + 1);
      }
      if (isMap) {
        let key;
        if (text[i] === "'" || text[i] === '"') {
          const k = this.scanQuoted(text, i, line);
          key = k.value;
          i = k.end;
        } else {
          const k = this.scanFlowPlainKey(text, i, line);
          key = k.value;
          i = k.end;
        }
        while (i < text.length && text[i] === " ") i++;
        if (i >= text.length || text[i] !== ":") {
          throw this.error("expected ':' in flow mapping", line.index + 1, i + 1);
        }
        i++;
        while (i < text.length && text[i] === " ") i++;
        if (i >= text.length || text[i] === "," || text[i] === close) {
          throw this.error("missing value in flow mapping", line.index + 1, i + 1);
        }
        const v = this.parseFlowValue(text, i, line);
        i = v.end;
        const canon = canonicalKey(key);
        if (seen.has(canon)) {
          throw this.error(`duplicate key '${String(key)}'`, line.index + 1, i + 1);
        }
        seen.set(canon, true);
        order.push([key, v.value]);
      } else {
        const v = this.parseFlowValue(text, i, line);
        i = v.end;
        values.push(v.value);
      }
      while (i < text.length && text[i] === " ") i++;
      if (i >= text.length) throw this.error("unterminated flow collection", line.index + 1, start + 1);
      if (text[i] === close) {
        i++;
        break;
      }
      if (text[i] === ",") {
        i++;
        continue;
      }
      throw this.error("expected ',' or closing bracket in flow collection", line.index + 1, i + 1);
    }
    return { value: isMap ? finalizeMap(order) : values, end: i };
  }

  parseFlowValue(text, start, line) {
    const c = text[start];
    if (c === "[" || c === "{") {
      const nested = this.parseFlow(text, start, line);
      return { value: nested.value, end: nested.end };
    }
    if (c === "'" || c === '"') return this.scanQuoted(text, start, line);
    let j = start;
    let sawSpace = false;
    while (j < text.length) {
      const ch = text[j];
      if (ch === "," || ch === "]" || ch === "}") break;
      if (ch === " ") {
        sawSpace = true;
        j++;
        continue;
      }
      if (ch === "#" && sawSpace) break;
      if (ch === ":" && (j + 1 >= text.length || text[j + 1] === " " || text[j + 1] === "," || text[j + 1] === "]" || text[j + 1] === "}")) {
        throw this.error("mapping value not allowed in flow scalar", line.index + 1, j + 1);
      }
      if (ch === "[" || ch === "{" || ch === "'" || ch === '"') {
        throw this.error("unexpected flow indicator in plain scalar", line.index + 1, j + 1);
      }
      sawSpace = false;
      j++;
    }
    const raw = text.slice(start, j).trim();
    if (raw === "") {
      throw this.error("missing value in flow collection", line.index + 1, start + 1);
    }
    this.assertPlainStart(raw, line.index + 1, start + 1);
    return { value: resolveScalar(raw, line.index + 1, start + 1), end: j };
  }

  scanFlowPlainKey(text, start, line) {
    let j = start;
    let sawSpace = false;
    while (j < text.length) {
      const c = text[j];
      if (c === ":") {
        const raw = text.slice(start, j).trim();
        if (raw === "") {
          throw this.error("missing key in flow mapping", line.index + 1, start + 1);
        }
        this.assertPlainStart(raw, line.index + 1, start + 1);
        return { value: resolveScalar(raw, line.index + 1, start + 1), end: j };
      }
      if (c === "," || c === "}" || c === "]" || c === "[" || c === "{") {
        throw this.error("expected ':' in flow mapping key", line.index + 1, j + 1);
      }
      if (c === " ") {
        sawSpace = true;
      } else {
        sawSpace = false;
      }
      if (c === "#" && sawSpace) {
        throw this.error("comment not allowed in flow mapping key", line.index + 1, j + 1);
      }
      j++;
    }
    throw this.error("expected ':' in flow mapping key", line.index + 1, start + 1);
  }
}

export function parseFrontMatter(source) {
  const lines = splitLines(source);
  if (lines.length === 0 || lines[0].text !== "---") {
    return { frontMatter: {}, body: source, raw: "", bodyOffset: 0 };
  }
  return new Parser(source, lines).parse();
}

// Parsed front matter carries source-order evidence; caller-constructed mappings use normal Object.entries order.
export function orderedMappingEntries(mapping) {
  const keys = mapping?.[ORDERED_KEYS];
  if (!Array.isArray(keys)) return Object.entries(mapping);
  return keys.map((key) => [key, mapping[key]]);
}
