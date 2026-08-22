const IMAGE_MIMES = new Set([
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/svg+xml",
]);

const pendingImages = new WeakMap();

function validBase64(value) {
  return /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value);
}

function decodedLength(value) {
  const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
  return (value.length / 4) * 3 - padding;
}

function appendWarning(warnings, slots, index, path) {
  slots[index] = { code: "W-UI-04", path };
  warnings.length = 0;
  for (const warning of slots) {
    if (warning !== undefined) warnings.push(warning);
  }
}

function markMissing(image, path, index, warnings, warningSlots) {
  pendingImages.delete(image);
  image.removeAttribute("src");
  image.removeAttribute("data-md-asset-ready");
  image.setAttribute("data-md-asset-missing", "");
  appendWarning(warnings, warningSlots, index, path);
}

function primitiveSet(view) {
  return typeof view?.atob === "function" &&
    typeof view?.Uint8Array === "function" &&
    typeof view?.Blob === "function" &&
    typeof view?.IntersectionObserver === "function" &&
    typeof view?.URL?.createObjectURL === "function";
}

export function hydrateImages(doc) {
  const app = doc?.querySelector?.("#mdhtml-app");
  const images = Array.from(app?.querySelectorAll?.("img") ?? []);
  const blocks = doc?.querySelectorAll?.('script[type="application/octet-stream"][data-path]') ?? [];
  const assets = new Map();
  for (const block of blocks) {
    const path = block.getAttribute("data-path");
    const previous = assets.get(path);
    if (previous !== undefined) previous.duplicate = true;
    else assets.set(path, { block, duplicate: false });
  }

  const warnings = [];
  const warningSlots = [];
  const view = doc?.defaultView;
  let observer = null;

  const finishMissing = (image, path, index) => {
    const state = pendingImages.get(image);
    if (state?.observer !== undefined) {
      try {
        state.observer.unobserve(image);
      } catch {}
    }
    markMissing(image, path, index, warnings, warningSlots);
  };

  const hydrateDelayed = (state) => {
    const { image, path, index, payload, mime } = state;
    if (!pendingImages.has(image)) return;
    try {
      const binary = view.atob(payload);
      const bytes = new view.Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
      const blob = new view.Blob([bytes], { type: mime });
      const url = view.URL.createObjectURL(blob);
      pendingImages.delete(image);
      image.setAttribute("src", url);
      image.setAttribute("data-md-asset-ready", "");
      state.observer.unobserve(image);
    } catch {
      finishMissing(image, path, index);
    }
  };

  for (let index = 0; index < images.length; index += 1) {
    const image = images[index];
    if (image.getAttribute("data-md-asset-ready") !== null ||
      image.getAttribute("data-md-asset-missing") !== null ||
      pendingImages.has(image)) continue;

    const path = image.getAttribute("data-md-asset-path");
    const asset = assets.get(path);
    const mime = asset?.block?.getAttribute("data-type");
    const payload = String(asset?.block?.textContent ?? "").replace(/[\t-\r ]/gu, "");
    if (asset === undefined || asset.duplicate || !IMAGE_MIMES.has(mime) || !validBase64(payload)) {
      markMissing(image, path, index, warnings, warningSlots);
      continue;
    }

    if (decodedLength(payload) < 32768) {
      image.setAttribute("src", `data:${mime};base64,${payload}`);
      image.setAttribute("data-md-asset-ready", "");
      continue;
    }

    if (!primitiveSet(view)) {
      markMissing(image, path, index, warnings, warningSlots);
      continue;
    }

    image.removeAttribute("src");
    if (observer === null) {
      try {
        observer = new view.IntersectionObserver((entries) => {
          for (const entry of entries) {
            if (entry.isIntersecting === true) {
              const state = pendingImages.get(entry.target);
              if (state !== undefined) hydrateDelayed(state);
            }
          }
        });
      } catch {
        markMissing(image, path, index, warnings, warningSlots);
        continue;
      }
    }

    const state = { image, path, index, payload, mime, observer };
    pendingImages.set(image, state);
    try {
      observer.observe(image);
    } catch {
      finishMissing(image, path, index);
    }
  }

  return { warnings, images };
}
