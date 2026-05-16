/**
 * Ferry popup — focused transfer control surface.
 */

import { checkHealth, revealPath, cancelJob, skipJob } from "./api.js";
import { runtimeAvailable, safeSendMessage } from "./runtime.js";

var statusDot = document.getElementById("status-dot");
var statusPill = document.getElementById("status-pill");
var statusPillText = document.getElementById("status-pill-text");
var openSettingsBtn = document.getElementById("open-settings");
var jobListEl = document.getElementById("job-list");
var clearBtn = document.getElementById("clear-activity");
var summaryActiveEl = document.getElementById("summary-active");
var summaryDoneEl = document.getElementById("summary-done");
var summaryErrorEl = document.getElementById("summary-error");
var signalCards = Array.from(document.querySelectorAll("[data-signal]"));
var refreshTimer = null;
var THEME_STORAGE_KEY = "ferryPopupThemeMode";
var systemThemeQuery = globalThis.matchMedia ? globalThis.matchMedia("(prefers-color-scheme: dark)") : null;

var state = {
  focus: "all",
  pinnedJobId: null,
  items: [],
  health: null,
  themeMode: "system",
};

function getStorage() {
  return globalThis.chrome && globalThis.chrome.storage && globalThis.chrome.storage.local;
}

function openExtensionSettingsPage() {
  var runtime = globalThis.chrome && globalThis.chrome.runtime;
  var tabs = globalThis.chrome && globalThis.chrome.tabs;
  if (runtime && typeof runtime.openOptionsPage === "function") {
    return runtime.openOptionsPage().catch(function() {
      if (runtime && typeof runtime.getURL === "function" && tabs && typeof tabs.create === "function") {
        return tabs.create({ url: runtime.getURL("settings.html") }).catch(function() {});
      }
    });
  }
  if (runtime && typeof runtime.getURL === "function" && tabs && typeof tabs.create === "function") {
    return tabs.create({ url: runtime.getURL("settings.html") }).catch(function() {});
  }
  return Promise.resolve();
}

function getResolvedTheme(themeMode) {
  if (themeMode === "light") return "light";
  if (themeMode === "dark") return "dark";
  return systemThemeQuery && systemThemeQuery.matches ? "dark" : "light";
}

function applyThemeMode(themeMode) {
  var nextMode = themeMode === "light" || themeMode === "dark" ? themeMode : "system";
  var resolvedTheme = getResolvedTheme(nextMode);
  state.themeMode = nextMode;
  if (document.body) {
    document.body.setAttribute("data-theme-mode", nextMode);
    document.body.setAttribute("data-theme", resolvedTheme);
  }
}

function loadThemeMode() {
  var storage = getStorage();
  if (!storage || !storage.get) {
    applyThemeMode("system");
    return Promise.resolve();
  }
  return storage.get(THEME_STORAGE_KEY).then(function(data) {
    applyThemeMode(data && data[THEME_STORAGE_KEY] ? data[THEME_STORAGE_KEY] : "system");
  }).catch(function() {
    applyThemeMode("system");
  });
}

function handleSystemThemeChange() {
  if (state.themeMode === "system") {
    applyThemeMode("system");
  }
}

function handleStorageChange(changes, areaName) {
  if (areaName !== "local" || !changes || !changes[THEME_STORAGE_KEY]) return;
  var nextValue = changes[THEME_STORAGE_KEY].newValue;
  applyThemeMode(nextValue || "system");
}

function applyHealthState(health) {
  state.health = health || null;
  if (state.health) {
    statusDot.className = "status-dot online";
    if (statusPillText) statusPillText.textContent = "Online";
    if (statusPill) statusPill.title = "Island desktop online";
  } else {
    statusDot.className = "status-dot offline";
    if (statusPillText) statusPillText.textContent = "Offline";
    if (statusPill) statusPill.title = "Island desktop not reachable";
  }
}

function refreshHealthState() {
  return checkHealth().then(function(health) {
    applyHealthState(health);
    return health;
  }).catch(function() {
    applyHealthState(null);
    return null;
  });
}

function formatTime(iso) {
  if (!iso) return "";
  var date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function escapeHtml(value) {
  return String(value || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function getStatusText(item) {
  if (item.state === "done") return "Saved";
  if (item.state === "error") return item.errorMessage || item.message || "Failed";
  if (item.state === "queued") return "Queued";
  if (item.state === "progress") {
    return item.speed ? item.speed + " • " + (item.eta || "calculating") : "Downloading";
  }
  return "Unknown";
}

function getMediaBadge(item) {
  if (item.mediaType === "audio") return "Audio";
  if (item.mediaType === "thumbnail") return "Thumb";
  return "Video";
}

function getMediaBadgeMarkup(item) {
  var icon = "";
  if (item.mediaType === "audio") {
    icon = '<svg viewBox="0 0 20 20" aria-hidden="true"><path fill="currentColor" d="M12.5 3.5a.75.75 0 0 1 1 .72v7.91a2.9 2.9 0 1 1-1.5-2.54V6.42l-3.5.93v5.78a2.9 2.9 0 1 1-1.5-2.54V6.77a.75.75 0 0 1 .56-.72l4.94-1.31Z" /></svg>';
  } else if (item.mediaType === "thumbnail") {
    icon = '<svg viewBox="0 0 20 20" aria-hidden="true"><path fill="currentColor" d="M4.5 3.75A1.75 1.75 0 0 0 2.75 5.5v9A1.75 1.75 0 0 0 4.5 16.25h11A1.75 1.75 0 0 0 17.25 14.5v-9A1.75 1.75 0 0 0 15.5 3.75h-11ZM7 7a1.5 1.5 0 1 1 0 3a1.5 1.5 0 0 1 0-3Zm7.25 6.25H5.76a.5.5 0 0 1-.37-.83l2.13-2.36a.5.5 0 0 1 .74-.02l1.46 1.46l2.1-2.62a.5.5 0 0 1 .77 0l2.05 2.56a.5.5 0 0 1-.39.81Z" /></svg>';
  } else {
    icon = '<svg viewBox="0 0 20 20" aria-hidden="true"><path fill="currentColor" d="M5 4.25A1.75 1.75 0 0 0 3.25 6v8A1.75 1.75 0 0 0 5 15.75h6A1.75 1.75 0 0 0 12.75 14V11.9l3.58 2.04a.75.75 0 0 0 1.12-.65V6.71a.75.75 0 0 0-1.12-.65l-3.58 2.04V6A1.75 1.75 0 0 0 11 4.25H5Z" /></svg>';
  }
  return '<span class="job-badge-icon" aria-hidden="true">' + icon + "</span>";
}

function getJobMetaLine(item) {
  var bits = [];
  if (item.qualityLabel) bits.push(item.qualityLabel);
  if (item.formatLabel) bits.push(item.formatLabel);
  if (item.state === "progress" && item.progress !== undefined && item.progress !== null) {
    bits.push(item.progress + "%");
  }
  return bits.join(" • ");
}

function getThumbnailMarkup(item) {
  if (item.thumbnailUrl) {
    return '<img class="job-thumb-image" src="' + escapeHtml(item.thumbnailUrl) + '" alt="" loading="lazy" referrerpolicy="no-referrer" />';
  }
  return '<div class="job-thumb-fallback" aria-hidden="true"><img src="icons/icon48.png" alt="" class="job-thumb-fallback-image" /></div>';
}

function getItemsForFocus(items, focus, pinnedJobId) {
  if (focus === "pinned") {
    return items.filter(function(item) {
      return item.jobId === pinnedJobId;
    });
  }
  if (focus === "active") {
    return items.filter(function(item) {
      return item.state === "queued" || item.state === "progress";
    });
  }
  if (focus === "done") {
    return items.filter(function(item) {
      return item.state === "done";
    });
  }
  if (focus === "attention") {
    return items.filter(function(item) {
      return item.state === "error" || item.state === "queued";
    });
  }
  return items;
}

function updateHero(items) {
  var activeCount = items.filter(function(item) { return item.state === "queued" || item.state === "progress"; }).length;
  var doneCount = items.filter(function(item) { return item.state === "done"; }).length;
  var issueCount = items.filter(function(item) { return item.state === "error"; }).length;
  if (summaryActiveEl) summaryActiveEl.textContent = String(activeCount);
  if (summaryDoneEl) summaryDoneEl.textContent = String(doneCount);
  if (summaryErrorEl) summaryErrorEl.textContent = String(issueCount);
}

function renderEmptyState(message, copy) {
  if (!jobListEl) return;
  jobListEl.innerHTML =
    '<div class="empty-state">' +
      '<div class="empty-state-mark" aria-hidden="true"><img src="icons/icon48.png" alt="" class="empty-state-image" /></div>' +
      '<div class="empty-state-title">' + escapeHtml(message) + '</div>' +
      '<div class="empty-state-copy">' + escapeHtml(copy) + '</div>' +
    '</div>';
}

function createJobCard(item) {
  var card = document.createElement("div");
  card.className = "job-card state-" + item.state + (item.jobId === state.pinnedJobId ? " is-focused" : "");
  card.id = "job-" + item.jobId;
  card.dataset.jobId = item.jobId;

  var badge = getMediaBadge(item);
  var statusText = getStatusText(item);
  var time = formatTime(item.updatedAt || item.createdAt);
  var progress = (item.progress !== undefined && item.progress !== null) ? item.progress : 0;
  var metaLine = getJobMetaLine(item);
  var actionMarkup = getJobActionMarkup(item);
  var progressReadout = item.state === "done" ? "Saved" : item.state === "error" ? "Issue" : progress + "%";

  var html = '<div class="job-card-layout">' +
      '<div class="job-thumb">' + getThumbnailMarkup(item) + '</div>' +
      '<div class="job-main">' +
        '<div class="job-topline">' +
          '<div class="job-badge-row">' +
            '<div class="job-badge badge-' + escapeHtml(item.mediaType || "video") + '" title="' + escapeHtml(badge) + '" aria-label="' + escapeHtml(badge) + '">' + getMediaBadgeMarkup(item) + '</div>' +
            '<div class="job-chip">' + escapeHtml(item.qualityLabel || item.formatLabel || "Waiting") + '</div>' +
          '</div>' +
          actionMarkup +
        '</div>' +
        '<div class="job-title" title="' + escapeHtml(item.title) + '">' + escapeHtml(item.title) + '</div>' +
        '<div class="job-meta">' + escapeHtml(metaLine || "Waiting for details") + '</div>' +
        '<div class="job-progress-container">' +
          '<div class="job-progress-bar" style="width: ' + progress + '%"></div>' +
        '</div>' +
        '<div class="job-bottomline">' +
          '<div class="job-status-line">' + escapeHtml(statusText) + '</div>' +
          '<div class="job-trailing">' +
            '<div class="job-progress-readout">' + escapeHtml(progressReadout) + '</div>' +
            '<div class="job-time-line">' + escapeHtml(time || "Waiting") + '</div>' +
          '</div>' +
        '</div>' +
      '</div>' +
    '</div>';

  card.innerHTML = html;
  card.onclick = function() {
    state.pinnedJobId = item.jobId;
    state.focus = "pinned";
    renderJobList(state.items);
  };

  var actionBtn = card.querySelector(".job-action-btn");
  if (actionBtn) {
    actionBtn.onclick = function(e) {
      e.stopPropagation();
      var action = actionBtn.dataset.action;
      if (action === "reveal") {
        revealPath(actionBtn.dataset.path).then(function(result) {
          if (result && result.ok === false) refreshHealthState();
        });
        return;
      }
      if (action === "cancel") {
        cancelJob(actionBtn.dataset.jobId).then(function(result) {
          if (result && result.ok === false) {
            return refreshHealthState().then(loadActivity);
          }
          return loadActivity();
        });
        return;
      }
      if (action === "skip") {
        skipJob(actionBtn.dataset.jobId).then(function(result) {
          if (result && result.ok === false) {
            return refreshHealthState().then(loadActivity);
          }
          return loadActivity();
        });
      }
    };
  }

  return card;
}

function getJobActionMarkup(item) {
  if (item.state === "done" && item.path) {
    return '<button class="job-action-btn job-reveal-btn" title="Open file" aria-label="Open file" data-action="reveal" data-path="' + escapeHtml(item.path) + '">' +
      '<svg viewBox="0 0 20 20" aria-hidden="true"><path fill="currentColor" d="M3.75 5A1.25 1.25 0 0 0 2.5 6.25v7.5A1.25 1.25 0 0 0 3.75 15h12.5a1.25 1.25 0 0 0 1.25-1.25V7.5a1.25 1.25 0 0 0-1.25-1.25H10L8.5 4.75A1.25 1.25 0 0 0 7.62 4.4H3.75Z" /></svg>' +
    '</button>';
  }
  if (item.state === "queued") {
    return '<button class="job-action-btn job-skip-btn" title="Skip job" aria-label="Skip job" data-action="skip" data-job-id="' + escapeHtml(item.jobId) + '">' +
      '<svg viewBox="0 0 20 20" aria-hidden="true"><path fill="currentColor" d="M4 5.2a.75.75 0 0 1 1.2-.6L10.1 8.3a.75.75 0 0 1 0 1.2L5.2 13.2A.75.75 0 0 1 4 12.6V5.2Zm6.75 0a.75.75 0 0 1 1.2-.6l4.9 3.7a.75.75 0 0 1 0 1.2l-4.9 3.7a.75.75 0 0 1-1.2-.6V5.2Z" /></svg>' +
    '</button>';
  }
  if (item.state === "progress") {
    return '<button class="job-action-btn job-cancel-btn" title="Cancel job" aria-label="Cancel job" data-action="cancel" data-job-id="' + escapeHtml(item.jobId) + '">' +
      '<svg viewBox="0 0 20 20" aria-hidden="true"><path fill="currentColor" d="M6 4.75h8A1.25 1.25 0 0 1 15.25 6v8A1.25 1.25 0 0 1 14 15.25H6A1.25 1.25 0 0 1 4.75 14V6A1.25 1.25 0 0 1 6 4.75Z" /></svg>' +
    '</button>';
  }
  return "";
}

function renderJobList(items) {
  if (!jobListEl) return;
  var safeItems = Array.isArray(items) ? items.slice() : [];
  safeItems.sort(function(a, b) {
    var aTime = new Date((a && (a.updatedAt || a.createdAt)) || 0).getTime();
    var bTime = new Date((b && (b.updatedAt || b.createdAt)) || 0).getTime();
    return bTime - aTime;
  });

  state.items = safeItems;
  updateHero(safeItems);

  var filtered = getItemsForFocus(safeItems, state.focus, state.pinnedJobId);
  if (!filtered.length) {
    if (state.focus === "active") {
      renderEmptyState("No live transfers", "New jobs will appear here as soon as Island starts processing.");
      return;
    }
    if (state.focus === "attention") {
      renderEmptyState("No issues right now", "When something fails or stalls, it will show up here.");
      return;
    }
    if (state.focus === "done") {
      renderEmptyState("No finished jobs yet", "Completed transfers will stay visible until the deck is cleared.");
      return;
    }
    if (state.focus === "pinned") {
      renderEmptyState("Nothing pinned", "Click any job card to pin it as your current focus.");
      return;
    }
    renderEmptyState("No recent activity", "Start a video, audio, or thumbnail download and Ferry will track it here.");
    return;
  }

  jobListEl.innerHTML = "";
  for (var i = 0; i < filtered.length; i++) {
    try {
      jobListEl.appendChild(createJobCard(filtered[i]));
    } catch (error) {
      console.error("[Ferry-Popup] Failed to render job card", filtered[i], error);
    }
  }
}

function updateJobCardInline(payload) {
  if (!payload || !payload.job_id) return;
  var card = document.getElementById("job-" + payload.job_id);
  if (!card) {
    loadActivity();
    return;
  }

  var bar = card.querySelector(".job-progress-bar");
  var percent = (payload.percent !== undefined && payload.percent !== null) ? payload.percent : 0;
  if (payload.event === "done") percent = 100;
  if (bar) bar.style.width = percent + "%";

  var line = card.querySelector(".job-status-line");
  var timeLine = card.querySelector(".job-time-line");
  var readout = card.querySelector(".job-progress-readout");
  var meta = card.querySelector(".job-meta");
  var trailing = card.querySelector(".job-progress-readout");

  if (line) {
    var now = new Date().toISOString();
    var timeStr = formatTime(now);

    if (payload.event === "progress") {
      var speed = payload.speed || "";
      var eta = payload.eta || "calculating";
      line.textContent = speed ? (speed + " • " + eta) : "Downloading";
      if (timeLine) timeLine.textContent = "Live";
      if (readout) readout.textContent = percent + "%";
      if (trailing) trailing.textContent = percent + "%";
      if (meta && percent > 0) {
        var text = meta.textContent || "";
        if (!/%$/.test(text)) {
          meta.textContent = text ? (text + " • " + percent + "%") : (percent + "%");
        }
      }
    } else if (payload.event === "done") {
      card.className = "job-card state-done" + (card.dataset.jobId === state.pinnedJobId ? " is-focused" : "");
      line.textContent = "Saved";
      if (timeLine) timeLine.textContent = timeStr;
      if (readout) readout.textContent = "Saved";
      setTimeout(loadActivity, 300);
    } else if (payload.event === "error") {
      card.className = "job-card state-error" + (card.dataset.jobId === state.pinnedJobId ? " is-focused" : "");
      line.textContent = "Failed";
      if (timeLine) timeLine.textContent = timeStr;
      if (readout) readout.textContent = "Issue";
      setTimeout(loadActivity, 300);
    }
  }
}

function processBuffer() {
  while (eventBuffer.length > 0) {
    updateJobCardInline(eventBuffer.shift());
  }
}

var isInitialLoading = true;
var isActivityLoading = false;
var eventBuffer = [];

function loadActivity() {
  if (isActivityLoading) return Promise.resolve();
  isActivityLoading = true;

  return safeSendMessage({ type: "GET_ACTIVITY" }).then(function(response) {
    var items = (response && response.items) || [];
    renderJobList(items);
    isActivityLoading = false;
    processBuffer();
  }).catch(function() {
    isActivityLoading = false;
  });
}

function applyPrimaryAction(health) {
  state.health = health || null;
}

function setFocus(nextFocus) {
  state.focus = nextFocus;
  renderJobList(state.items);
}

function init() {
  loadThemeMode();

  if (runtimeAvailable()) {
    var runtime = globalThis.chrome && globalThis.chrome.runtime;
    if (runtime && runtime.onMessage) {
      runtime.onMessage.addListener(function(message) {
        if (!message) return;
        if (message.type === "WS_EVENT") {
          updateJobCardInline(message.payload);
          return;
        }
        if (message.type === "ACTIVITY_UPDATED") {
          loadActivity();
        }
      });
    }
  }

  var storageApi = globalThis.chrome && globalThis.chrome.storage;
  if (storageApi && storageApi.onChanged && typeof storageApi.onChanged.addListener === "function") {
    storageApi.onChanged.addListener(handleStorageChange);
  }

  signalCards.forEach(function(card) {
    card.addEventListener("click", function() {
      setFocus(card.dataset.signal === "attention" ? "attention" : card.dataset.signal || "all");
    });
  });

  if (systemThemeQuery) {
    if (typeof systemThemeQuery.addEventListener === "function") {
      systemThemeQuery.addEventListener("change", handleSystemThemeChange);
    } else if (typeof systemThemeQuery.addListener === "function") {
      systemThemeQuery.addListener(handleSystemThemeChange);
    }
  }

  if (openSettingsBtn) {
    openSettingsBtn.onclick = function() {
      openExtensionSettingsPage();
    };
  }

  if (clearBtn) {
    clearBtn.onclick = function() {
      state.pinnedJobId = null;
      setFocus("all");
      safeSendMessage({ type: "STOP_WS" }).then(loadActivity);
    };
  }

  loadActivity().then(function() {
    isInitialLoading = false;
  });

  refreshTimer = setInterval(loadActivity, 2500);

  document.addEventListener("visibilitychange", function() {
    if (document.visibilityState === "visible") loadActivity();
  });

  refreshHealthState().then(function(health) {
    applyPrimaryAction(health);
  });
}

init();
