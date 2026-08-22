import { BASE_CSS, EDITORIAL_CSS, TECHNICAL_CSS } from "./styles.generated.js";

export function runtimeCss(theme) {
  return `${BASE_CSS}${theme === "editorial" ? EDITORIAL_CSS : TECHNICAL_CSS}`;
}

export function mountStyles(doc, theme) {
  const selected = theme === "editorial" ? "editorial" : "technical";
  doc.documentElement.setAttribute("data-mdhtml-preset", selected);
  const existing = doc.getElementById("mdhtml-runtime-style");
  if (existing) return existing;
  const style = doc.createElement("style");
  style.setAttribute("id", "mdhtml-runtime-style");
  style.textContent = runtimeCss(selected);
  doc.head.appendChild(style);
  return style;
}
