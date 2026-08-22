const ACTIONS = [
  ["copy", "Copy"],
  ["view", "View Markdown"],
  ["download", "Download"],
  ["theme", "Theme"],
];

function fallbackCopy(doc, text) {
  const textarea = doc.createElement("textarea");
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.value = text;
  doc.body.appendChild(textarea);
  try {
    textarea.select();
    doc.execCommand("copy");
  } finally {
    textarea.remove();
  }
}

export function copyText(doc, text) {
  const view = doc.defaultView;
  const writeText = view?.navigator?.clipboard?.writeText;
  if (typeof writeText !== "function") {
    try {
      fallbackCopy(doc, text);
    } catch {}
    return;
  }

  try {
    const result = writeText.call(view.navigator.clipboard, text);
    if (result && typeof result.catch === "function") {
      const handled = result.catch(() => {
        try {
          fallbackCopy(doc, text);
        } catch {}
      });
      if (handled && typeof handled.catch === "function") handled.catch(() => {});
    }
  } catch {
    try {
      fallbackCopy(doc, text);
    } catch {}
  }
}

function button(doc, action, label) {
  const element = doc.createElement("button");
  element.setAttribute("type", "button");
  element.setAttribute("data-md-action", action);
  element.textContent = label;
  return element;
}

function currentMode(select) {
  return select.value || "smart";
}

function downloadSource(doc, storedSource) {
  const view = doc.defaultView;
  const blob = new view.Blob([storedSource], { type: "text/markdown" });
  const url = view.URL.createObjectURL(blob);
  const anchor = doc.createElement("a");
  anchor.setAttribute("href", url);
  anchor.setAttribute("download", "document.md");
  doc.body.appendChild(anchor);
  try {
    anchor.click();
  } finally {
    anchor.remove();
    view.URL.revokeObjectURL(url);
  }
}

export function mountChrome(doc, storedSource, projectMarkdown) {
  const existing = doc.getElementById("mdhtml-toolbar");
  if (existing) return existing;

  const root = doc.documentElement;
  const app = doc.getElementById("mdhtml-app");
  const toolbar = doc.createElement("nav");
  toolbar.setAttribute("id", "mdhtml-toolbar");
  toolbar.setAttribute("aria-label", "Document controls");

  const mode = doc.createElement("select");
  mode.setAttribute("id", "mdhtml-copy-mode");
  mode.setAttribute("aria-label", "Copy mode");
  for (const value of ["smart", "full", "body"]) {
    const option = doc.createElement("option");
    option.setAttribute("value", value);
    option.textContent = value;
    mode.appendChild(option);
  }
  mode.value = "smart";
  toolbar.appendChild(mode);

  const buttons = new Map();
  for (const [action, label] of ACTIONS) {
    const control = button(doc, action, label);
    buttons.set(action, control);
    toolbar.appendChild(control);
  }

  const dialog = doc.createElement("dialog");
  dialog.setAttribute("id", "mdhtml-source-view");
  const pre = doc.createElement("pre");
  const close = doc.createElement("button");
  close.setAttribute("type", "button");
  close.textContent = "Close";
  dialog.append(pre, close);

  app.parentNode.insertBefore(toolbar, app);
  app.parentNode.insertBefore(dialog, app);
  root.setAttribute("data-mdhtml-theme", "system");

  buttons.get("copy").addEventListener("click", () => {
    copyText(doc, projectMarkdown(storedSource, currentMode(mode)));
  });
  buttons.get("view").addEventListener("click", () => {
    pre.textContent = projectMarkdown(storedSource, currentMode(mode));
    dialog.showModal();
  });
  buttons.get("download").addEventListener("click", () => {
    downloadSource(doc, storedSource);
  });
  buttons.get("theme").addEventListener("click", () => {
    const themes = ["system", "light", "dark"];
    const current = root.getAttribute("data-mdhtml-theme");
    const index = themes.indexOf(current);
    root.setAttribute("data-mdhtml-theme", themes[(index + 1) % themes.length]);
  });
  close.addEventListener("click", () => dialog.close());

  return toolbar;
}
