#!/usr/bin/env node

const args = process.argv.slice(2);

function getArg(name, fallback = null) {
  const idx = args.indexOf(name);
  if (idx === -1) return fallback;
  return args[idx + 1] ?? fallback;
}

function hasFlag(name) {
  return args.includes(name);
}

const base = getArg("--base", "http://127.0.0.1:49152");
const videoUrl = getArg("--url", "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
const shouldQueue = hasFlag("--queue");

async function callJson(path, method = "GET", body) {
  const response = await fetch(`${base}${path}`, {
    method,
    headers: body ? { "content-type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined
  });

  let data = null;
  try {
    data = await response.json();
  } catch {
    data = null;
  }

  if (!response.ok) {
    const reason = data?.error || `${response.status} ${response.statusText}`;
    throw new Error(`${method} ${path} failed: ${reason}`);
  }

  return data;
}

async function pollStatus(jobId) {
  const maxChecks = 20;
  for (let i = 0; i < maxChecks; i += 1) {
    const status = await callJson(`/status/${jobId}`);
    const summary = `${status.status} (${status.progress}%)`;
    console.log(`status[${i + 1}/${maxChecks}]: ${summary}`);

    if (status.status === "done") {
      console.log(`output: ${status.output_path || "(not provided)"}`);
      return;
    }

    if (status.status === "error") {
      throw new Error(`job error: ${status.message || "unknown"}`);
    }

    await new Promise((resolve) => setTimeout(resolve, 1500));
  }

  throw new Error("status polling timed out");
}

async function main() {
  console.log(`base: ${base}`);

  const health = await callJson("/health");
  console.log(`health: ${health.status} (v${health.version}, port ${health.port})`);

  const formats = await callJson("/formats", "POST", { url: videoUrl });
  const list = Array.isArray(formats.formats) ? formats.formats : [];
  console.log(`formats: ${list.length}`);
  if (list.length > 0) {
    console.log(`first format: ${list[0].label} [${list[0].format}/${list[0].quality}]`);
  }

  if (!shouldQueue) {
    console.log("queue: skipped (pass --queue to test /download + /status)");
    return;
  }

  if (list.length === 0) {
    throw new Error("cannot queue download without at least one format option");
  }

  const first = list[0];
  const download = await callJson("/download", "POST", {
    url: videoUrl,
    title: "Smoke Test",
    format: first.format,
    quality: first.quality
  });

  console.log(`job_id: ${download.job_id}`);
  await pollStatus(download.job_id);
}

main().catch((error) => {
  console.error(`smoke failed: ${error.message}`);
  process.exit(1);
});
