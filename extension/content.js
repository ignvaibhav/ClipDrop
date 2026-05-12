/**
 * ClipDrop content script — Injects a download button and panel on YouTube watch pages.
 *
 * This is the primary user-facing interface. The popup is a fallback path.
 * Styles are loaded from content.css via manifest.json.
 *
 * NOTE: Content scripts cannot use ES module imports in MV3.
 * API and runtime helpers are inlined here. Shared modules (api.js, runtime.js,
 * constants.js) are used by background.js and popup.js which run as modules.
 */

// ---------------------------------------------------------------------------
// Constants (inlined — content scripts can't import modules)
// ---------------------------------------------------------------------------

const API_BASE = "http://127.0.0.1:49152";
const BUTTON_ID = "clipdrop-injected-button";
const PANEL_ID = "clipdrop-inline-panel";
const INJECT_DEBOUNCE_MS = 200;
const MAX_INJECTION_FRAMES = 180;

// ---------------------------------------------------------------------------
// Runtime helpers (inlined)
// ---------------------------------------------------------------------------

function runtimeAvailable() {
  return Boolean(globalThis.chrome?.runtime?.id);
}

async function safeSendMessage(message) {
  const runtime = globalThis.chrome?.runtime;
  if (!runtimeAvailable() || typeof runtime?.sendMessage !== "function") return null;
  try {
    return await runtime.sendMessage(message);
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// API helpers (inlined)
// ---------------------------------------------------------------------------

async function checkHealth() {
  const res = await fetch(`${API_BASE}/health`);
  if (!res.ok) throw new Error(`health failed (${res.status})`);
  return res.json();
}

async function loadFormats(url) {
  const res = await fetch(`${API_BASE}/formats`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url }),
  });
  if (!res.ok) {
    const error = await res.json().catch(() => ({}));
    throw new Error(error.error || `formats failed (${res.status})`);
  }
  const data = await res.json();
  return data.formats || [];
}

async function queueDownload(payload) {
  const res = await fetch(`${API_BASE}/download`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const error = await res.json().catch(() => ({}));
    throw new Error(error.error || `download failed (${res.status})`);
  }
  return res.json();
}

async function fetchJobStatus(jobId) {
  const res = await fetch(`${API_BASE}/status/${encodeURIComponent(jobId)}`);
  if (!res.ok) throw new Error(`status failed (${res.status})`);
  return res.json();
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let activeJobId = null;
let wsBound = false;
let currentVideoUrl = "";
let panelAnchorButton = null;
let prefetchUrl = "";
let prefetchState = { loading: false, health: null, formats: [], error: null };
let injectTimer = null;
let statusPollTimer = null;

// ---------------------------------------------------------------------------
// YouTube page detection
// ---------------------------------------------------------------------------

function isWatchPage() {
  try {
    const url = new URL(window.location.href);
    return url.hostname.includes("youtube.com") && url.pathname === "/watch";
  } catch {
    return false;
  }
}

function getVideoContext() {
  const url = window.location.href;
  const titleEl =
    document.querySelector("h1.ytd-watch-metadata yt-formatted-string") ||
    document.querySelector("h1.title");
  const title = titleEl?.textContent?.trim() || document.title.replace(" - YouTube", "");
  return { url, title };
}

// ---------------------------------------------------------------------------
// Progress / status display
// ---------------------------------------------------------------------------

function setStatus(panel, text, ok = true, show = false) {
  const box = panel.querySelector("[data-clipdrop=progress-box]");
  const textEl = panel.querySelector("[data-clipdrop=progress-text]");
  if (!box || !textEl) return;
  if (!show) return;
  box.style.display = "block";
  textEl.textContent = ok ? text : `Error: ${text}`;
}

function setProgress(panel, progress, speed, eta) {
  const box = panel.querySelector("[data-clipdrop=progress-box]");
  const text = panel.querySelector("[data-clipdrop=progress-text]");
  if (!box || !text) return;

  box.style.display = "block";
  const value = Number.isFinite(progress) ? Math.max(0, Math.min(100, progress)) : 0;
  const parts = value > 0 ? [`Queue… ${value}%`] : ["Queue…"];
  if (speed) parts.push(speed);
  if (eta) parts.push(`ETA ${eta}`);
  text.textContent = parts.join(" • ");
}

function resetProgressBox(panel) {
  const box = panel.querySelector("[data-clipdrop=progress-box]");
  const text = panel.querySelector("[data-clipdrop=progress-text]");
  if (!box || !text) return;
  box.style.display = "none";
  text.textContent = "Queue…";
}

// ---------------------------------------------------------------------------
// Status polling (fallback when WS misses events)
// ---------------------------------------------------------------------------

function stopStatusPolling() {
  if (statusPollTimer) {
    clearInterval(statusPollTimer);
    statusPollTimer = null;
  }
}

function startStatusPolling(panel, jobId) {
  stopStatusPolling();
  if (!jobId) return;
  const downloadBtn = panel.querySelector("[data-clipdrop=download-btn]");

  statusPollTimer = setInterval(async () => {
    try {
      const status = await fetchJobStatus(jobId);
      if (status?.status === "in_progress") {
        setProgress(panel, status.progress ?? 0, null, null);
        return;
      }
      if (status?.status === "done") {
        setProgress(panel, 100, null, null);
        const fileName = status.output_path ? status.output_path.split(/[\\/]/).pop() : null;
        setStatus(panel, fileName ? `Done ✓ ${fileName}` : "Done ✓ Saved to Downloads", true, true);
        if (downloadBtn) downloadBtn.disabled = false;
        stopStatusPolling();
        return;
      }
      if (status?.status === "error") {
        setStatus(panel, status.message || "Download failed", false, true);
        if (downloadBtn) downloadBtn.disabled = false;
        stopStatusPolling();
      }
    } catch {
      if (downloadBtn) downloadBtn.disabled = false;
      stopStatusPolling();
    }
  }, 2000);
}

// ---------------------------------------------------------------------------
// Prefetch video data
// ---------------------------------------------------------------------------

function applyPrefetchStateToPanel(panel) {
  if (prefetchState.loading) {
    setStatus(panel, "Loading video options…", true, false);
    return;
  }

  if (prefetchState.error) {
    setStatus(panel, `Desktop app not reachable: ${prefetchState.error}`, false, false);
    return;
  }

  if (prefetchState.health) {
    setStatus(panel, `Desktop app online (v${prefetchState.health.version})`, true, false);
  }

  renderFormats(panel, prefetchState.formats || []);
  if (!prefetchState.formats?.length) {
    setStatus(panel, "No downloadable formats found for this video", false, false);
  }
}

async function prefetchVideoData(url) {
  if (!url || prefetchState.loading || prefetchUrl === url) return;
  prefetchUrl = url;
  prefetchState = { loading: true, health: null, formats: [], error: null };

  try {
    const [health, formats] = await Promise.all([checkHealth(), loadFormats(url)]);
    prefetchState.health = health;
    prefetchState.formats = formats;
    prefetchState.error = null;
    await safeSendMessage({ type: "ENSURE_WS" });
  } catch (error) {
    prefetchState.error = error?.message || "unknown error";
  } finally {
    prefetchState.loading = false;
    const panel = document.getElementById(PANEL_ID);
    if (panel && panel.style.display !== "none") {
      applyPrefetchStateToPanel(panel);
    }
  }
}

// ---------------------------------------------------------------------------
// Clip slider
// ---------------------------------------------------------------------------

function secondsToTimestamp(rawSeconds) {
  const total = Math.max(0, Math.floor(rawSeconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

function getPlayerDurationSeconds() {
  const playerVideo = document.querySelector("video");
  const duration = Number(playerVideo?.duration || 0);
  return Number.isFinite(duration) && duration > 0 ? Math.floor(duration) : 0;
}

function clipPayload(panel) {
  if (panel.dataset.clipTouched !== "1") return null;
  const startSeconds = Number(panel.dataset.clipStartSeconds || 0);
  const endSeconds = Number(panel.dataset.clipEndSeconds || 0);
  if (!Number.isFinite(startSeconds) || !Number.isFinite(endSeconds) || endSeconds <= startSeconds) {
    return null;
  }
  return { start: secondsToTimestamp(startSeconds), end: secondsToTimestamp(endSeconds) };
}

function setupClipSlider(panel) {
  const box = panel.querySelector("[data-clipdrop=clip-slider-box]");
  const track = panel.querySelector("[data-clipdrop=clip-track]");
  const fill = panel.querySelector("[data-clipdrop=clip-fill]");
  const startThumb = panel.querySelector("[data-clipdrop=clip-start-thumb]");
  const endThumb = panel.querySelector("[data-clipdrop=clip-end-thumb]");
  const readout = panel.querySelector("[data-clipdrop=clip-readout]");
  if (!box || !track || !fill || !startThumb || !endThumb || !readout) return;
  panel.dataset.clipTouched = "0";

  const duration = getPlayerDurationSeconds();
  if (!duration) {
    panel.dataset.clipStartSeconds = "0";
    panel.dataset.clipEndSeconds = "0";
    box.style.display = "none";
    readout.textContent = "Clip slider unavailable until video duration is loaded.";
    return;
  }

  box.style.display = "grid";
  let start = 0;
  let end = duration;

  const clamp = (value, min, max) => Math.max(min, Math.min(max, value));
  const pct = (value) => (value / duration) * 100;

  const render = () => {
    const startPct = pct(start);
    const endPct = pct(end);
    startThumb.style.left = `${startPct}%`;
    endThumb.style.left = `${endPct}%`;
    fill.style.left = `${startPct}%`;
    fill.style.width = `${Math.max(0.5, endPct - startPct)}%`;
    panel.dataset.clipStartSeconds = String(start);
    panel.dataset.clipEndSeconds = String(end);
    readout.textContent = `${secondsToTimestamp(start)} → ${secondsToTimestamp(end)}`;
  };

  const secondsFromClientX = (clientX) => {
    const rect = track.getBoundingClientRect();
    const ratio = clamp((clientX - rect.left) / Math.max(1, rect.width), 0, 1);
    return Math.round(ratio * duration);
  };

  const beginDrag = (which) => (downEvent) => {
    downEvent.preventDefault();
    panel.dataset.clipTouched = "1";
    const onMove = (moveEvent) => {
      const cursorSeconds = secondsFromClientX(moveEvent.clientX);
      if (which === "start") {
        start = clamp(cursorSeconds, 0, end - 1);
      } else {
        end = clamp(cursorSeconds, start + 1, duration);
      }
      render();
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  startThumb.onpointerdown = beginDrag("start");
  endThumb.onpointerdown = beginDrag("end");

  track.onclick = (event) => {
    panel.dataset.clipTouched = "1";
    const cursorSeconds = secondsFromClientX(event.clientX);
    const distToStart = Math.abs(cursorSeconds - start);
    const distToEnd = Math.abs(cursorSeconds - end);
    if (distToStart <= distToEnd) {
      start = clamp(cursorSeconds, 0, end - 1);
    } else {
      end = clamp(cursorSeconds, start + 1, duration);
    }
    render();
  };

  render();
}

// ---------------------------------------------------------------------------
// Format selector
// ---------------------------------------------------------------------------

function parseSelectedPreset(panel) {
  const select = panel.querySelector("[data-clipdrop=preset]");
  if (!select?.value) return { format: "mp4", quality: "best", format_id: null };
  try {
    return JSON.parse(select.value);
  } catch {
    return { format: "mp4", quality: "best", format_id: null };
  }
}

function renderFormats(panel, formats) {
  const select = panel.querySelector("[data-clipdrop=preset]");
  if (!select) return;
  select.innerHTML = "";
  for (const format of formats) {
    if (!format?.label) continue;
    const option = document.createElement("option");
    option.textContent = format.label;
    option.value = JSON.stringify({
      format: format.format || "mp4",
      quality: format.quality || "best",
      format_id: format.format_id || null,
    });
    select.appendChild(option);
  }
}

// ---------------------------------------------------------------------------
// WebSocket listener
// ---------------------------------------------------------------------------

function ensureWsListener() {
  if (wsBound) return;
  const onMessage = globalThis.chrome?.runtime?.onMessage;
  if (!runtimeAvailable() || !onMessage?.addListener) return;

  try {
    onMessage.addListener((message) => {
      if (message?.type !== "WS_EVENT") return;
      const payload = message.payload;
      if (!payload || payload.job_id !== activeJobId) return;

      const panel = document.getElementById(PANEL_ID);
      if (!panel) return;

      if (payload.event === "progress") {
        setProgress(panel, payload.percent ?? 0, payload.speed, payload.eta);
        return;
      }

      const downloadBtn = panel.querySelector("[data-clipdrop=download-btn]");
      if (payload.event === "done") {
        setProgress(panel, 100, null, null);
        const fileName = payload.path ? payload.path.split(/[\\/]/).pop() : null;
        setStatus(panel, fileName ? `Done ✓ ${fileName}` : "Done ✓ Saved to Downloads", true, true);
        if (downloadBtn) downloadBtn.disabled = false;
        stopStatusPolling();
        return;
      }

      if (payload.event === "error") {
        setStatus(panel, payload.message || "Download failed", false, true);
        if (downloadBtn) downloadBtn.disabled = false;
        stopStatusPolling();
      }
    });
    wsBound = true;
  } catch {
    wsBound = false;
  }
}

// ---------------------------------------------------------------------------
// Panel creation (uses CSS classes from content.css)
// ---------------------------------------------------------------------------

function createPanel() {
  const panel = document.createElement("div");
  panel.id = PANEL_ID;
  panel.style.display = "none";

  panel.innerHTML = `
    <div class="clipdrop-panel-header">
      <div class="clipdrop-panel-title">ClipDrop</div>
      <button type="button" data-clipdrop="close" class="clipdrop-close-btn" aria-label="Close">×</button>
    </div>
    <div data-clipdrop="title" class="clipdrop-video-title">Loading video…</div>

    <label class="clipdrop-label">Quality</label>
    <select data-clipdrop="preset" class="clipdrop-select"></select>

    <details class="clipdrop-clip-details">
      <summary class="clipdrop-clip-summary">Clip (optional)</summary>
      <div data-clipdrop="clip-slider-box" class="clipdrop-clip-slider-box">
        <div data-clipdrop="clip-readout" class="clipdrop-clip-readout">Loading clip slider…</div>
        <div data-clipdrop="clip-track" class="clipdrop-clip-track">
          <div class="clipdrop-clip-track-bg"></div>
          <div data-clipdrop="clip-fill" class="clipdrop-clip-fill"></div>
          <button type="button" data-clipdrop="clip-start-thumb" class="clipdrop-clip-thumb"></button>
          <button type="button" data-clipdrop="clip-end-thumb" class="clipdrop-clip-thumb"></button>
        </div>
      </div>
    </details>

    <button data-clipdrop="download-btn" class="clipdrop-download-btn">Download</button>

    <div data-clipdrop="progress-box" class="clipdrop-progress-box">
      <div data-clipdrop="progress-text" class="clipdrop-progress-text">Queue…</div>
    </div>
  `;

  panel.querySelector("[data-clipdrop=close]")?.addEventListener("click", () => {
    panel.style.display = "none";
    resetProgressBox(panel);
    stopStatusPolling();
  });

  panel.querySelector("[data-clipdrop=download-btn]")?.addEventListener("click", async () => {
    const context = getVideoContext();
    const preset = parseSelectedPreset(panel);
    const downloadBtn = panel.querySelector("[data-clipdrop=download-btn]");

    const payload = {
      url: context.url,
      title: context.title || null,
      format: preset.format,
      quality: preset.quality,
      format_id: preset.format_id || null,
    };

    const clip = clipPayload(panel);
    if (clip) payload.clip = clip;

    if (downloadBtn) downloadBtn.disabled = true;
    setStatus(panel, "Queue…", true, true);

    try {
      const result = await queueDownload(payload);
      activeJobId = result.job_id;
      setProgress(panel, 0, null, null);
      setStatus(panel, "Queue… Download started", true, true);
      await safeSendMessage({ type: "TRACK_JOB", jobId: activeJobId });
      startStatusPolling(panel, activeJobId);
    } catch (error) {
      setStatus(panel, error?.message || "Failed to queue download", false, true);
      if (downloadBtn) downloadBtn.disabled = false;
    }
  });

  return panel;
}

// ---------------------------------------------------------------------------
// Panel positioning
// ---------------------------------------------------------------------------

function positionPanelFromButton(panel, button) {
  const rect = button.getBoundingClientRect();
  const viewportW = window.innerWidth;
  const panelWidth = 380;
  const margin = 12;

  let left = rect.right - panelWidth;
  left = Math.max(margin, Math.min(left, viewportW - panelWidth - margin));
  const top = rect.bottom + 8;

  panel.style.left = `${Math.max(margin, left)}px`;
  panel.style.top = `${Math.max(margin, top)}px`;
}

function refreshPanelPosition() {
  const panel = document.getElementById(PANEL_ID);
  if (!panel || panel.style.display === "none") return;
  if (!panelAnchorButton || !panelAnchorButton.isConnected) return;
  positionPanelFromButton(panel, panelAnchorButton);
}

// ---------------------------------------------------------------------------
// Panel initialization
// ---------------------------------------------------------------------------

async function initializePanel(panel) {
  const context = getVideoContext();
  const titleEl = panel.querySelector("[data-clipdrop=title]");
  if (titleEl) titleEl.textContent = context.title || context.url;
  setupClipSlider(panel);
  resetProgressBox(panel);
  currentVideoUrl = context.url;
  if (prefetchUrl !== context.url && !prefetchState.loading) {
    void prefetchVideoData(context.url);
  }
  applyPrefetchStateToPanel(panel);
}

// ---------------------------------------------------------------------------
// Button creation
// ---------------------------------------------------------------------------

function createButton() {
  const button = document.createElement("button");
  button.id = BUTTON_ID;
  button.type = "button";
  button.textContent = "ClipDrop";
  button.setAttribute("aria-label", "Download with ClipDrop");
  // Styles applied via content.css #clipdrop-injected-button

  button.addEventListener("click", async () => {
    const actionBar =
      button.closest("#top-level-buttons-computed") ||
      document.querySelector("#top-level-buttons-computed") ||
      document.querySelector("#menu #top-level-buttons-computed");
    if (!actionBar) return;

    const panel = ensurePanel(actionBar);
    if (!panel) return;
    panelAnchorButton = button;
    positionPanelFromButton(panel, button);

    const opening = panel.style.display === "none";
    panel.style.display = opening ? "block" : "none";

    if (opening) {
      await initializePanel(panel);
    }
  });

  return button;
}

// ---------------------------------------------------------------------------
// DOM injection
// ---------------------------------------------------------------------------

function ensurePanel(actionBar) {
  let panel = document.getElementById(PANEL_ID);
  if (!panel || !panel.isConnected) {
    panel = createPanel();
    actionBar.insertAdjacentElement("afterend", panel);
  }
  return panel;
}

function findActionBar() {
  return (
    document.querySelector("#top-level-buttons-computed") ||
    document.querySelector("#menu #top-level-buttons-computed") ||
    document.querySelector("ytd-watch-metadata #top-level-buttons-computed") ||
    document.querySelector("#actions #top-level-buttons-computed")
  );
}

function cleanupInjectedElements() {
  document.getElementById(BUTTON_ID)?.remove();
  document.getElementById(PANEL_ID)?.remove();
  currentVideoUrl = "";
  prefetchUrl = "";
  prefetchState = { loading: false, health: null, formats: [], error: null };
  stopStatusPolling();
}

function injectButtonAndPanel() {
  if (!isWatchPage()) {
    cleanupInjectedElements();
    return;
  }

  const actionBar = findActionBar();
  if (!actionBar) return;

  if (!document.getElementById(BUTTON_ID)) {
    actionBar.appendChild(createButton());
  }

  ensurePanel(actionBar);
  const url = window.location.href;
  if (prefetchUrl !== url && !prefetchState.loading) {
    void prefetchVideoData(url);
  }
}

/** Debounced injection to prevent excessive calls from MutationObserver. */
function scheduleInject() {
  if (injectTimer) clearTimeout(injectTimer);
  injectTimer = setTimeout(() => {
    injectButtonAndPanel();
  }, INJECT_DEBOUNCE_MS);
}

/** Immediate injection loop using rAF for SPA navigation. */
function startImmediateInjectionLoop() {
  let frames = 0;
  const tick = () => {
    injectButtonAndPanel();
    frames += 1;
    if (document.getElementById(BUTTON_ID) || frames >= MAX_INJECTION_FRAMES) return;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

ensureWsListener();

// Debounced MutationObserver for DOM changes
const observer = new MutationObserver(scheduleInject);
observer.observe(document.documentElement, { childList: true, subtree: true });

// Initial injection
injectButtonAndPanel();

// YouTube SPA navigation events
window.addEventListener("yt-navigate-start", () => {
  cleanupInjectedElements();
});

window.addEventListener("yt-navigate-finish", () => {
  injectButtonAndPanel();
  startImmediateInjectionLoop();
});

window.addEventListener("yt-page-data-updated", () => {
  injectButtonAndPanel();
  startImmediateInjectionLoop();
});

startImmediateInjectionLoop();

// Reposition panel on scroll/resize
window.addEventListener("scroll", refreshPanelPosition, { passive: true });
window.addEventListener("resize", refreshPanelPosition);
