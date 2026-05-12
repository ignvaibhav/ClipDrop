/**
 * ClipDrop popup — Activity feed only.
 *
 * The primary download UX is the inline panel in content.js.
 * This popup only shows recent activity and desktop app status.
 */

import { MAX_DISPLAYED_ACTIVITY } from "./constants.js";
import { checkHealth } from "./api.js";
import { runtimeAvailable, safeSendMessage } from "./runtime.js";

// ---------------------------------------------------------------------------
// DOM elements
// ---------------------------------------------------------------------------

const statusLine = document.getElementById("status-line");
const activityListEl = document.getElementById("activity-list");

// ---------------------------------------------------------------------------
// Activity feed
// ---------------------------------------------------------------------------

function formatActivityTime(iso) {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function renderActivity(items) {
  if (!activityListEl) return;
  if (!Array.isArray(items) || !items.length) {
    activityListEl.textContent = "No recent downloads.";
    return;
  }

  activityListEl.innerHTML = items
    .slice(0, MAX_DISPLAYED_ACTIVITY)
    .map((item) => {
      const label = item?.message || "Update";
      const stamp = formatActivityTime(item?.at);
      const stateIcon =
        item?.state === "done" ? "✓" : item?.state === "error" ? "✗" : "⋯";
      return `<div>${stateIcon} ${label}${stamp ? ` <span style="color:#8f8f8f">(${stamp})</span>` : ""}</div>`;
    })
    .join("");
}

async function loadRecentActivity() {
  const response = await safeSendMessage({ type: "GET_ACTIVITY" });
  renderActivity(response?.items || []);
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

async function init() {
  await loadRecentActivity();

  try {
    const health = await checkHealth();
    statusLine.textContent = `Desktop app online (v${health.version})`;
    statusLine.style.color = "#8fd68f";
  } catch {
    statusLine.textContent = "Desktop app not reachable. Start ClipDrop desktop.";
    statusLine.style.color = "#ff9a9a";
  }
}

// Listen for live WS events to refresh activity
if (runtimeAvailable()) {
  const onMessage = globalThis.chrome?.runtime?.onMessage;
  if (onMessage?.addListener) {
    try {
      onMessage.addListener((message) => {
        if (message?.type !== "WS_EVENT") return;
        const payload = message.payload;
        if (!payload) return;
        if (payload.event === "done" || payload.event === "error") {
          void loadRecentActivity();
        }
      });
    } catch {
      // ignore
    }
  }
}

init();
