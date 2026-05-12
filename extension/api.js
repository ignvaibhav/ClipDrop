/**
 * API client for communicating with the ClipDrop local desktop companion.
 * Consolidates fetch wrappers previously duplicated in content.js and popup.js.
 */

import { API_BASE } from "./constants.js";

/**
 * Check if the desktop companion is reachable.
 * @returns {Promise<object>} Health response with { status, version, port }.
 */
export async function checkHealth() {
  const res = await fetch(`${API_BASE}/health`);
  if (!res.ok) {
    throw new Error(`health failed (${res.status})`);
  }
  return res.json();
}

/**
 * Fetch available download formats for a video URL.
 * @param {string} url - The video URL.
 * @returns {Promise<Array>} List of format option objects.
 */
export async function loadFormats(url) {
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

/**
 * Submit a download job to the queue.
 * @param {object} payload - Download request payload.
 * @returns {Promise<object>} Response with { job_id }.
 */
export async function queueDownload(payload) {
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

/**
 * Poll the status of a specific download job.
 * @param {string} jobId - The job ID to check.
 * @returns {Promise<object>} Status response.
 */
export async function fetchJobStatus(jobId) {
  const res = await fetch(
    `${API_BASE}/status/${encodeURIComponent(jobId)}`
  );
  if (!res.ok) {
    throw new Error(`status failed (${res.status})`);
  }
  return res.json();
}
