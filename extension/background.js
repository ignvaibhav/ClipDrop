/**
 * ClipDrop background service worker.
 *
 * Manages WebSocket connection to the desktop companion, broadcasts download
 * events to content/popup contexts, and creates desktop notifications.
 */

import { API_BASE, WS_URL, ACTIVITY_KEY, MAX_ACTIVITY_ITEMS } from "./constants.js";
import { runtimeAvailable } from "./runtime.js";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let socket = null;
let reconnectTimer = null;
let reconnectDelay = 2000;
const activeJobs = new Set();
let shouldKeepSocket = false;
const notificationTargets = new Map();

/** Maximum reconnect backoff (ms). */
const MAX_RECONNECT_DELAY = 30_000;
/** Auto-expire notification targets after this many ms. */
const NOTIFICATION_TARGET_TTL = 5 * 60 * 1000;

// ---------------------------------------------------------------------------
// Messaging
// ---------------------------------------------------------------------------

function broadcastToExtension(message) {
  if (!runtimeAvailable()) return;
  const runtime = globalThis.chrome?.runtime;
  if (typeof runtime?.sendMessage !== "function") return;
  try {
    runtime.sendMessage(message).catch(() => {
      // Receiver may not be open; ignore.
    });
  } catch {
    // Extension context might be reloading.
  }
}

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

function notify(title, message, options = {}) {
  if (!runtimeAvailable()) return;
  if (!globalThis.chrome?.notifications?.create) return;

  const notificationId =
    options.notificationId ||
    `clipdrop-${Date.now()}-${Math.random().toString(16).slice(2)}`;

  if (options.path) {
    notificationTargets.set(notificationId, {
      path: options.path,
      createdAt: Date.now(),
    });
  }

  try {
    const iconUrl = globalThis.chrome?.runtime?.getURL
      ? globalThis.chrome.runtime.getURL("icons/icon128.png")
      : "icons/icon128.png";

    globalThis.chrome.notifications.create(
      notificationId,
      {
        type: "basic",
        iconUrl,
        title,
        message,
      },
      () => {
        const err = globalThis.chrome?.runtime?.lastError;
        if (err) {
          console.warn("notification create failed:", err.message || err);
        }
      }
    );
  } catch (error) {
    console.warn("notification create threw:", error);
  }
}

/** Remove expired notification targets to prevent memory leaks. */
function cleanupNotificationTargets() {
  const cutoff = Date.now() - NOTIFICATION_TARGET_TTL;
  for (const [id, entry] of notificationTargets) {
    if (entry.createdAt < cutoff) {
      notificationTargets.delete(id);
    }
  }
}

async function revealPath(path) {
  if (!path) return;
  try {
    await fetch(`${API_BASE}/reveal`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path }),
    });
  } catch {
    // ignore errors when backend is unavailable
  }
}

// ---------------------------------------------------------------------------
// Activity log
// ---------------------------------------------------------------------------

async function readActivity() {
  const storage = globalThis.chrome?.storage?.local;
  if (!storage?.get) return [];
  try {
    const data = await storage.get(ACTIVITY_KEY);
    return Array.isArray(data?.[ACTIVITY_KEY]) ? data[ACTIVITY_KEY] : [];
  } catch {
    return [];
  }
}

async function writeActivity(items) {
  const storage = globalThis.chrome?.storage?.local;
  if (!storage?.set) return;
  try {
    await storage.set({ [ACTIVITY_KEY]: items.slice(0, MAX_ACTIVITY_ITEMS) });
  } catch {
    // ignore storage failures during reload/invalidation
  }
}

async function pushActivity(entry) {
  const now = new Date().toISOString();
  const next = [{ ...entry, at: now }, ...(await readActivity())];
  await writeActivity(next);
}

// ---------------------------------------------------------------------------
// Badge
// ---------------------------------------------------------------------------

function setBadge(text) {
  const action = globalThis.chrome?.action;
  if (!action?.setBadgeText || !action?.setBadgeBackgroundColor) return;
  action.setBadgeText({ text });
  action.setBadgeBackgroundColor({ color: "#111111" });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function filenameFromPath(path) {
  if (!path || typeof path !== "string") return null;
  const normalized = path.replaceAll("\\", "/");
  const segments = normalized.split("/");
  return segments[segments.length - 1] || null;
}

// ---------------------------------------------------------------------------
// WebSocket with exponential backoff
// ---------------------------------------------------------------------------

function scheduleReconnect() {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
  }
  if (shouldKeepSocket && runtimeAvailable()) {
    reconnectTimer = setTimeout(() => {
      void connectSocket();
    }, reconnectDelay);
    // Exponential backoff: 2s → 4s → 8s → 16s → 30s max
    reconnectDelay = Math.min(reconnectDelay * 2, MAX_RECONNECT_DELAY);
  }
}

/** Reset backoff delay after a successful connection. */
function resetReconnectDelay() {
  reconnectDelay = 2000;
}

async function apiReachable() {
  try {
    const res = await fetch(`${API_BASE}/health`, { method: "GET", cache: "no-store" });
    return res.ok;
  } catch {
    return false;
  }
}

async function connectSocket() {
  if (!runtimeAvailable()) return;
  if (!shouldKeepSocket) return;
  if (
    socket &&
    (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)
  ) {
    return;
  }

  if (!(await apiReachable())) {
    scheduleReconnect();
    return;
  }

  socket = new WebSocket(WS_URL);

  socket.addEventListener("open", () => {
    resetReconnectDelay();
  });

  socket.addEventListener("message", (event) => {
    let payload = null;
    try {
      payload = JSON.parse(event.data);
    } catch {
      return;
    }

    if (!payload?.job_id) return;

    broadcastToExtension({ type: "WS_EVENT", payload });

    // Clean up stale notification targets periodically
    cleanupNotificationTargets();

    if (payload.event === "done") {
      const name = filenameFromPath(payload.path);
      notify("ClipDrop", name ? `Done ✓ ${name}` : "Done ✓ Saved to Downloads", {
        notificationId: `clipdrop-done-${payload.job_id}`,
        path: payload.path || null,
      });
      void pushActivity({
        state: "done",
        message: name ? `Done ✓ ${name}` : "Done ✓ Saved to Downloads",
        jobId: payload.job_id,
      });
      activeJobs.delete(payload.job_id);
      setBadge(activeJobs.size ? String(activeJobs.size) : "");
      return;
    }

    if (payload.event === "error") {
      notify("ClipDrop", payload.message || "Download failed", {
        notificationId: `clipdrop-error-${payload.job_id}`,
      });
      void pushActivity({
        state: "error",
        message: payload.message || "Download failed",
        jobId: payload.job_id,
      });
      activeJobs.delete(payload.job_id);
      setBadge(activeJobs.size ? String(activeJobs.size) : "");
      return;
    }

    if (payload.event === "progress" && activeJobs.has(payload.job_id)) {
      setBadge(String(activeJobs.size));
    }
  });

  socket.addEventListener("close", () => {
    scheduleReconnect();
  });

  socket.addEventListener("error", () => {
    try {
      socket?.close();
    } catch {
      // no-op
    }
  });
}

// ---------------------------------------------------------------------------
// Message handlers
// ---------------------------------------------------------------------------

globalThis.chrome?.runtime?.onMessage?.addListener((message, _sender, sendResponse) => {
  if (message?.type === "TRACK_JOB") {
    if (message.jobId) {
      activeJobs.add(message.jobId);
      void pushActivity({
        state: "queued",
        message: "Queue… Download started",
        jobId: message.jobId,
      });
      shouldKeepSocket = true;
      setBadge(String(activeJobs.size));
      void connectSocket();
    }
    sendResponse({ ok: true });
    return true;
  }

  if (message?.type === "ENSURE_WS") {
    shouldKeepSocket = true;
    void connectSocket();
    sendResponse({ ok: true });
    return true;
  }

  if (message?.type === "STOP_WS") {
    shouldKeepSocket = false;
    activeJobs.clear();
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    try {
      socket?.close();
    } catch {
      // no-op
    }
    socket = null;
    resetReconnectDelay();
    setBadge("");
    sendResponse({ ok: true });
    return true;
  }

  if (message?.type === "GET_ACTIVITY") {
    void readActivity().then((items) => {
      sendResponse({ ok: true, items });
    });
    return true;
  }

  return false;
});

// ---------------------------------------------------------------------------
// Notification click handler
// ---------------------------------------------------------------------------

globalThis.chrome?.notifications?.onClicked?.addListener((notificationId) => {
  const entry = notificationTargets.get(notificationId);
  if (entry?.path) {
    void revealPath(entry.path);
  }
  notificationTargets.delete(notificationId);
  globalThis.chrome?.notifications?.clear?.(notificationId);
});
