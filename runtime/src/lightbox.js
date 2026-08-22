const mountedSources = new WeakSet();

function eligibleImages(images) {
  return images.filter((image) => image.getAttribute("data-md-asset-missing") === null);
}

function readyImages(images) {
  return images.filter((image) =>
    image.getAttribute("data-md-asset-missing") === null &&
    image.getAttribute("data-md-asset-ready") !== null);
}

function copyAttribute(target, source, name) {
  const value = source.getAttribute(name);
  if (value === null) target.removeAttribute(name);
  else target.setAttribute(name, value);
}

export function mountLightbox(doc, images) {
  const eligible = eligibleImages(Array.from(images ?? []));
  if (eligible.length === 0) return null;

  const existing = doc.getElementById("mdhtml-lightbox");
  if (existing) return existing;

  const dialog = doc.createElement("dialog");
  dialog.setAttribute("id", "mdhtml-lightbox");
  const displayed = doc.createElement("img");
  const counter = doc.createElement("output");
  counter.setAttribute("data-md-lightbox-counter", "");
  const previous = doc.createElement("button");
  previous.setAttribute("type", "button");
  previous.setAttribute("data-md-lightbox-action", "previous");
  previous.textContent = "Previous";
  const next = doc.createElement("button");
  next.setAttribute("type", "button");
  next.setAttribute("data-md-lightbox-action", "next");
  next.textContent = "Next";
  const close = doc.createElement("button");
  close.setAttribute("type", "button");
  close.setAttribute("data-md-lightbox-action", "close");
  close.textContent = "Close";
  dialog.append(displayed, counter, previous, next, close);
  doc.body.appendChild(dialog);

  let current = null;

  function renderCurrent(source) {
    const gallery = readyImages(eligible);
    const index = gallery.indexOf(source);
    if (index < 0) return false;
    current = source;
    copyAttribute(displayed, source, "src");
    copyAttribute(displayed, source, "alt");
    copyAttribute(displayed, source, "title");
    counter.textContent = `${index + 1} / ${gallery.length}`;
    return true;
  }

  function navigate(step) {
    const gallery = readyImages(eligible);
    if (current === null || gallery.length === 0) return;
    const index = gallery.indexOf(current);
    const start = index < 0 ? 0 : index;
    renderCurrent(gallery[(start + step + gallery.length) % gallery.length]);
  }

  function onSourceClick(event) {
    const source = event.currentTarget;
    if (source.getAttribute("data-md-asset-ready") === null) return;
    if (renderCurrent(source)) dialog.showModal();
  }

  for (const source of eligible) {
    if (mountedSources.has(source)) continue;
    mountedSources.add(source);
    source.addEventListener("click", onSourceClick);
  }

  previous.addEventListener("click", () => navigate(-1));
  next.addEventListener("click", () => navigate(1));
  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("keydown", (event) => {
    if (event.key === "ArrowLeft") navigate(-1);
    if (event.key === "ArrowRight") navigate(1);
  });

  let pointerStart = null;
  displayed.addEventListener("pointerdown", (event) => {
    pointerStart = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
    };
  });
  displayed.addEventListener("pointerup", (event) => {
    if (pointerStart === null || pointerStart.id !== event.pointerId) return;
    const horizontal = event.clientX - pointerStart.x;
    const vertical = event.clientY - pointerStart.y;
    pointerStart = null;
    if (Math.abs(horizontal) < 40 || Math.abs(horizontal) <= Math.abs(vertical)) return;
    navigate(horizontal < 0 ? 1 : -1);
  });

  if (doc.defaultView.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    dialog.setAttribute("data-md-reduced-motion", "");
  }

  return dialog;
}
