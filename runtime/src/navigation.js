function decodedHash(view) {
  const hash = view?.location?.hash ?? "";
  try {
    return decodeURIComponent(hash.startsWith("#") ? hash.slice(1) : hash);
  } catch {
    return null;
  }
}

export function mountToc(doc, headings, config) {
  if (config === false) return null;
  const depth = Number.isInteger(config?.depth) && config.depth >= 1 && config.depth <= 6 ? config.depth : 3;
  const position = config?.position === "inline" || config?.position === "side" ? config.position : "side";
  const visible = (headings ?? []).filter((heading) => Number.isInteger(heading?.level) && heading.level >= 1 && heading.level <= depth);
  if (visible.length === 0) return null;
  const existing = doc.getElementById("mdhtml-toc");
  if (existing) return existing;

  const toolbar = doc.getElementById("mdhtml-toolbar");
  if (!toolbar?.parentNode) return null;
  const toc = doc.createElement("nav");
  toc.setAttribute("id", "mdhtml-toc");
  toc.setAttribute("aria-label", "Table of contents");
  toc.setAttribute("data-md-toc-position", position);
  const list = doc.createElement("ol");
  const links = [];
  for (const heading of visible) {
    const item = doc.createElement("li");
    const link = doc.createElement("a");
    link.setAttribute("href", `#${heading.id}`);
    link.setAttribute("data-level", String(heading.level));
    link.textContent = heading.text;
    item.appendChild(link);
    list.appendChild(item);
    links.push(link);
  }
  toc.appendChild(list);
  toolbar.parentNode.insertBefore(toc, toolbar.nextSibling);

  const sync = () => {
    const active = decodedHash(doc.defaultView);
    for (const link of links) {
      if (active !== null && link.getAttribute("href") === `#${active}`) {
        link.setAttribute("aria-current", "location");
      } else {
        link.removeAttribute("aria-current");
      }
    }
  };
  doc.defaultView.addEventListener("hashchange", sync);
  sync();
  return toc;
}
