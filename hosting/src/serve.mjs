import {
  CACHE_CONTENT,
  CACHE_SHORT_ID,
  longUrl,
  textResponse,
} from "./contract.mjs";
import { robotsTxtResponse, securityHeaders } from "./origin.mjs";

const decoder = new TextDecoder();
const bytesToText = (bytes) => decoder.decode(bytes);

const LONG_URL = /^\/d\/([^/]+)\/([^/]+)$/;
const SHORT_ID = /^\/([^/]+)$/;

export async function serve(request, { store, denyList, baseUrl }) {
  const { pathname } = new URL(request.url);

  if (pathname === "/robots.txt") {
    return robotsTxtResponse();
  }

  const longMatch = LONG_URL.exec(pathname);
  if (longMatch !== null) {
    const html = await store.readDocument(longMatch[2]);
    if (html === null) return textResponse(404, "not found");
    const text = bytesToText(html);
    return new Response(text, {
      status: 200,
      headers: {
        "Content-Type": "text/html; charset=utf-8",
        "Cache-Control": CACHE_CONTENT,
        ...securityHeaders(text),
      },
    });
  }

  const shortMatch = SHORT_ID.exec(pathname);
  if (shortMatch !== null && shortMatch[1] !== "robots.txt") {
    const mapping = await store.resolveId(shortMatch[1]);
    if (mapping === null) return textResponse(404, "not found");
    return new Response(null, {
      status: 308,
      headers: {
        Location: longUrl(baseUrl, mapping.toolchainId, mapping.sha256),
        "Cache-Control": CACHE_SHORT_ID,
      },
    });
  }

  return textResponse(404, "not found");
}
