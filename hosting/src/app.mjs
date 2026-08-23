import { API_PATH } from "./contract.mjs";
import { createDenyList } from "./origin.mjs";
import { createRateLimiter, handlePublish } from "./publish.mjs";
import { serve } from "./serve.mjs";
import { createStore, memoryBackend } from "./storage.mjs";
import { buildWasm, loadToolchain } from "./toolchain.mjs";

export async function createApp({ repoRoot, wasmPath, baseUrl }) {
  const resolvedWasmPath =
    wasmPath ?? (await buildWasm({ repoRoot })).wasmPath;
  const toolchain = await loadToolchain({
    repoRoot,
    wasmPath: resolvedWasmPath,
  });
  const backend = memoryBackend();
  const store = createStore({ backend, toolchainId: toolchain.toolchainId });
  const denyList = createDenyList({ backend });
  const rateLimiter = createRateLimiter();
  return {
    store,
    denyList,
    toolchain,
    async handle(request, { ip = "127.0.0.1" } = {}) {
      const url = new URL(request.url);
      if (url.pathname === API_PATH && request.method === "POST") {
        return handlePublish(request, {
          toolchain,
          store,
          denyList,
          rateLimiter,
          ip,
          baseUrl,
        });
      }
      return serve(request, { store, denyList, baseUrl });
    },
  };
}
