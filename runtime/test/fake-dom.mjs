export class FakeElement {
  constructor(tagName, ownerDocument) {
    this.tagName = tagName.toUpperCase();
    this.ownerDocument = ownerDocument;
    this.children = [];
    this.parentNode = null;
    this.attributes = {};
    this.listeners = {};
    this.style = {};
    this.disabled = false;
    this._textContent = "";
    this.value = "";
    this._innerHTML = "";
  }

  get textContent() {
    return this._textContent;
  }

  set textContent(value) {
    this._textContent = String(value);
  }

  get innerHTML() {
    return this._innerHTML;
  }

  set innerHTML(value) {
    this._innerHTML = String(value);
    for (const child of this.children) child.parentNode = null;
    this.children = [];
    if (this._innerHTML === "") return;
    for (const match of this._innerHTML.matchAll(/<img\b([^>]*)>/giu)) {
      const image = new FakeElement("img", this.ownerDocument);
      for (const attribute of match[1].matchAll(/([:\w-]+)(?:="([^"]*)")?/gu)) {
        image.setAttribute(attribute[1], decodeAttribute(attribute[2] ?? ""));
      }
      this.appendChild(image);
    }
  }

  get nextSibling() {
    if (!this.parentNode) return null;
    const index = this.parentNode.children.indexOf(this);
    return this.parentNode.children[index + 1] ?? null;
  }

  append(...nodes) {
    for (const node of nodes) this.appendChild(node);
  }

  appendChild(node) {
    node.parentNode = this;
    this.children.push(node);
    return node;
  }

  insertBefore(node, reference) {
    node.parentNode = this;
    const index = this.children.indexOf(reference);
    this.children.splice(index < 0 ? this.children.length : index, 0, node);
    return node;
  }

  removeChild(node) {
    const index = this.children.indexOf(node);
    if (index >= 0) this.children.splice(index, 1);
    node.parentNode = null;
    return node;
  }

  remove() {
    this.parentNode?.removeChild(this);
  }

  setAttribute(name, value) {
    this.attributes[name] = String(value);
    if (name === "id") this.id = String(value);
    if (name === "value") this.value = String(value);
    if (name === "disabled") this.disabled = true;
  }

  getAttribute(name) {
    return this.attributes[name] ?? null;
  }

  removeAttribute(name) {
    delete this.attributes[name];
    if (name === "disabled") this.disabled = false;
  }

  hasAttribute(name) {
    return Object.prototype.hasOwnProperty.call(this.attributes, name);
  }

  addEventListener(type, listener) {
    this.listeners[type] = listener;
  }

  dispatchEvent(type, payload = {}) {
    this.listeners[type]?.({ currentTarget: this, type, ...payload });
  }

  listenerCount(type) {
    return this.listeners[type] ? 1 : 0;
  }

  querySelector(selector) {
    if (selector === "pre") return this.children.find((child) => child.tagName === "PRE") ?? null;
    if (selector === "button") return this.children.find((child) => child.tagName === "BUTTON") ?? null;
    return this.ownerDocument.find(this, selector);
  }

  querySelectorAll(selector) {
    return this.ownerDocument.findAll(this, selector);
  }

  select() {
    this.selected = true;
  }

  click() {
    this.clicked = true;
    this.dispatchEvent("click");
  }

  showModal() {
    this.open = true;
    this.showModalCalled = true;
    this.showModalCalls = (this.showModalCalls ?? 0) + 1;
  }

  close() {
    this.open = false;
    this.closeCalled = true;
    this.closeCalls = (this.closeCalls ?? 0) + 1;
  }
}

function decodeAttribute(value) {
  return value
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

export class FakeDocument {
  constructor({ clipboard, reducedMotion = false } = {}) {
    this.documentElement = new FakeElement("html", this);
    this.head = new FakeElement("head", this);
    this.body = new FakeElement("body", this);
    this.documentElement.append(this.head, this.body);
    this.created = [];
    const url = {
      createObjectURL: (blob) => {
        url.blob = blob;
        return "blob:document";
      },
      revokeObjectURL: (urlValue) => {
        url.revoked = urlValue;
      },
    };
    const listeners = {};
    const mediaQueries = [];
    const intersectionObservers = [];
    class FakeIntersectionObserver {
      constructor(callback) {
        this.callback = callback;
        this.observed = [];
        this.unobserved = [];
        intersectionObservers.push(this);
      }

      observe(element) {
        if (!this.observed.includes(element)) this.observed.push(element);
      }

      unobserve(element) {
        this.unobserved.push(element);
        this.observed = this.observed.filter((entry) => entry !== element);
      }

      trigger(entries) {
        this.callback(entries, this);
      }
    }
    this.defaultView = {
      navigator: clipboard ? { clipboard } : {},
      location: { hash: "" },
      atob: globalThis.atob,
      Uint8Array,
      IntersectionObserver: FakeIntersectionObserver,
      intersectionObservers,
      mediaQueries,
      matchMedia(query) {
        mediaQueries.push(query);
        return { matches: reducedMotion };
      },
      Blob: class FakeBlob {
        constructor(parts, options) {
          this.parts = parts;
          this.type = options.type;
        }
      },
      URL: url,
      addEventListener(type, listener) {
        listeners[type] = listeners[type] ?? [];
        listeners[type].push(listener);
      },
      dispatchEvent(type) {
        for (const listener of listeners[type] ?? []) listener({ type, currentTarget: this });
      },
      listenerCount(type) {
        return (listeners[type] ?? []).length;
      },
    };
  }

  createElement(tagName) {
    const element = new FakeElement(tagName, this);
    this.created.push(element);
    return element;
  }

  execCommand(command) {
    this.command = command;
    return true;
  }

  querySelector(selector) {
    return this.find(this.documentElement, selector);
  }

  querySelectorAll(selector) {
    return this.findAll(this.documentElement, selector);
  }

  find(root, selector) {
    return this.findAll(root, selector)[0] ?? null;
  }

  findAll(root, selector) {
    const matches = this.selectorMatcher(selector);
    const found = [];
    const visit = (node) => {
      for (const child of node.children) {
        if (matches(child)) found.push(child);
        visit(child);
      }
    };
    visit(root);
    return found;
  }

  // One matcher per selector kind this fake DOM supports; computed once per
  // findAll call instead of re-testing every alternative against every node.
  selectorMatcher(selector) {
    const actionMatch = selector.match(/^button\[data-md-action="([^"]+)"\]$/u);
    if (actionMatch) return (el) => el.tagName === "BUTTON" && el.getAttribute("data-md-action") === actionMatch[1];
    const lightboxActionMatch = selector.match(/^button\[data-md-lightbox-action="([^"]+)"\]$/u);
    if (lightboxActionMatch) {
      return (el) => el.tagName === "BUTTON" && el.getAttribute("data-md-lightbox-action") === lightboxActionMatch[1];
    }
    if (selector.startsWith("#")) {
      const id = selector.slice(1);
      return (el) => el.id === id;
    }
    if (selector === 'script[type="application/octet-stream"][data-path]') {
      return (el) => el.tagName === "SCRIPT" && el.getAttribute("type") === "application/octet-stream" && el.getAttribute("data-path") !== null;
    }
    if (selector === "output[data-md-lightbox-counter]") {
      return (el) => el.tagName === "OUTPUT" && el.hasAttribute("data-md-lightbox-counter");
    }
    const tag = { img: "IMG", dialog: "DIALOG", output: "OUTPUT", pre: "PRE", button: "BUTTON", select: "SELECT", a: "A", style: "STYLE" }[selector];
    if (tag) return (el) => el.tagName === tag;
    return () => false;
  }

  getElementById(id) {
    return this.find(this.documentElement, `#${id}`);
  }
}

export function appDocument(options = {}) {
  const doc = new FakeDocument(options);
  const app = doc.createElement("main");
  app.setAttribute("id", "mdhtml-app");
  doc.body.appendChild(app);
  doc.app = app;
  return doc;
}

export function documentFor(source, options = {}) {
  const doc = appDocument();
  if (options.format !== false) doc.documentElement.setAttribute("data-mdhtml", "1.0");
  const sourceElement = doc.createElement("script");
  sourceElement.setAttribute("id", "mdhtml-source");
  sourceElement.setAttribute("type", "text/markdown");
  sourceElement.textContent = source;
  doc.body.appendChild(sourceElement);
  const app = doc.getElementById("mdhtml-app");
  const markdownScripts = options.markdownScripts ?? [sourceElement];
  const apps = options.apps ?? [app];
  const querySelectorAll = doc.querySelectorAll.bind(doc);
  doc.querySelectorAll = (selector) => {
    if (selector === 'script[type="text/markdown"]') return markdownScripts;
    if (selector === "#mdhtml-app") return apps;
    return querySelectorAll(selector);
  };
  doc.sourceElement = sourceElement;
  doc.app = app;
  return doc;
}
