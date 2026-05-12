/**
 * Chrome runtime helpers shared across ClipDrop extension scripts.
 * Provides safe wrappers that handle extension context invalidation gracefully.
 */

/**
 * Check whether the extension runtime is still available.
 * Returns false if the extension has been reloaded or unloaded.
 */
export function runtimeAvailable() {
  return Boolean(globalThis.chrome?.runtime?.id);
}

/**
 * Send a message to the background service worker safely.
 * Returns null if the runtime is unavailable or the message fails.
 */
export async function safeSendMessage(message) {
  const runtime = globalThis.chrome?.runtime;
  if (!runtimeAvailable() || typeof runtime?.sendMessage !== "function") {
    return null;
  }
  try {
    return await runtime.sendMessage(message);
  } catch {
    return null;
  }
}
