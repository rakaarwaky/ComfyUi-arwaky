/**
 * ComfyUI Desktop Logs — Floating log panel extension.
 *
 * Adds a toggleable floating panel that streams backend logs
 * (ROCm, Python, ComfyUI) from the Rust launcher's log file
 * via the ComfyUI-Desktop-Logs Python extension HTTP endpoints.
 */

import { app } from "../../scripts/app.js";

const POLL_MS = 1000;
const MAX_LINES = 500;

let panelEl = null;
let contentEl = null;
let isVisible = false;
let lastId = -1;
let pollTimer = null;
let autoScroll = true;
let filterText = "";

// ── Create floating panel ──────────────────────────────────────────────────

function createPanel() {
  if (panelEl) return;

  panelEl = document.createElement("div");
  panelEl.id = "desktop-logs-panel";
  panelEl.innerHTML = `
    <style>
      #desktop-logs-panel {
        position: fixed;
        bottom: 20px;
        right: 20px;
        width: 700px;
        height: 400px;
        background: #1a1a2e;
        border: 1px solid #333;
        border-radius: 8px;
        font-family: 'JetBrains Mono', 'Fira Code', monospace;
        font-size: 12px;
        color: #e0e0e0;
        z-index: 99999;
        display: none;
        flex-direction: column;
        box-shadow: 0 8px 32px rgba(0,0,0,0.6);
        resize: both;
        overflow: hidden;
      }
      #desktop-logs-panel.visible {
        display: flex;
      }
      #desktop-logs-panel .logs-header {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 8px 12px;
        background: #16213e;
        border-bottom: 1px solid #333;
        border-radius: 8px 8px 0 0;
        cursor: move;
        user-select: none;
      }
      #desktop-logs-panel .logs-header .title {
        font-weight: bold;
        color: #7c3aed;
        flex: 1;
      }
      #desktop-logs-panel .logs-header button {
        background: #333;
        border: none;
        color: #e0e0e0;
        padding: 4px 8px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 11px;
      }
      #desktop-logs-panel .logs-header button:hover {
        background: #555;
      }
      #desktop-logs-panel .logs-header button.active {
        background: #7c3aed;
        color: white;
      }
      #desktop-logs-panel .logs-toolbar {
        display: flex;
        align-items: center;
        gap: 6px;
        padding: 4px 12px;
        background: #0f3460;
        border-bottom: 1px solid #333;
      }
      #desktop-logs-panel .logs-toolbar input {
        flex: 1;
        background: #1a1a2e;
        border: 1px solid #444;
        color: #e0e0e0;
        padding: 3px 8px;
        border-radius: 4px;
        font-size: 11px;
        font-family: inherit;
      }
      #desktop-logs-panel .logs-toolbar .count {
        color: #888;
        font-size: 10px;
      }
      #desktop-logs-panel .logs-content {
        flex: 1;
        overflow-y: auto;
        padding: 8px 12px;
        line-height: 1.5;
      }
      #desktop-logs-panel .logs-content .log-line {
        white-space: pre-wrap;
        word-break: break-all;
      }
      #desktop-logs-panel .logs-content .log-line.stdout {
        color: #a0a0a0;
      }
      #desktop-logs-panel .logs-content .log-line.stderr {
        color: #f87171;
      }
      #desktop-logs-panel .logs-content .log-line.launcher {
        color: #7c3aed;
      }
      #desktop-logs-panel .logs-content .log-line.highlight {
        background: rgba(124, 58, 237, 0.2);
      }
      #desktop-logs-panel .logs-content .log-line.filtered-out {
        display: none;
      }
    </style>
    <div class="logs-header">
      <span class="title">Backend Logs</span>
      <button id="logs-btn-clear">Clear</button>
      <button id="logs-btn-autoscroll" class="active">Auto</button>
      <button id="logs-btn-close">X</button>
    </div>
    <div class="logs-toolbar">
      <input id="logs-filter" type="text" placeholder="Filter logs..." />
      <span class="count" id="logs-count">0 lines</span>
    </div>
    <div class="logs-content" id="logs-content"></div>
  `;

  document.body.appendChild(panelEl);

  contentEl = panelEl.querySelector("#logs-content");
  const filterInput = panelEl.querySelector("#logs-filter");
  const btnClose = panelEl.querySelector("#logs-btn-close");
  const btnClear = panelEl.querySelector("#logs-btn-clear");
  const btnAuto = panelEl.querySelector("#logs-btn-autoscroll");

  // Drag functionality
  const header = panelEl.querySelector(".logs-header");
  let isDragging = false;
  let dragOffsetX = 0;
  let dragOffsetY = 0;

  header.addEventListener("mousedown", (e) => {
    if (e.target.tagName === "BUTTON") return;
    isDragging = true;
    dragOffsetX = e.clientX - panelEl.offsetLeft;
    dragOffsetY = e.clientY - panelEl.offsetTop;
    e.preventDefault();
  });

  document.addEventListener("mousemove", (e) => {
    if (!isDragging) return;
    panelEl.style.left = (e.clientX - dragOffsetX) + "px";
    panelEl.style.top = (e.clientY - dragOffsetY) + "px";
    panelEl.style.right = "auto";
    panelEl.style.bottom = "auto";
  });

  document.addEventListener("mouseup", () => {
    isDragging = false;
  });

  btnClose.addEventListener("click", () => {
    togglePanel();
  });

  btnClear.addEventListener("click", () => {
    contentEl.innerHTML = "";
    lastId = -1;
    updateCount();
  });

  btnAuto.addEventListener("click", () => {
    autoScroll = !autoScroll;
    btnAuto.classList.toggle("active", autoScroll);
  });

  filterInput.addEventListener("input", (e) => {
    filterText = e.target.value.toLowerCase();
    applyFilter();
  });

  // Scroll detection — disable auto-scroll when user scrolls up
  contentEl.addEventListener("scroll", () => {
    const atBottom =
      contentEl.scrollHeight - contentEl.scrollTop - contentEl.clientHeight < 30;
    if (!atBottom && autoScroll) {
      autoScroll = false;
      btnAuto.classList.remove("active");
    }
  });
}

// ── Panel controls ─────────────────────────────────────────────────────────

function togglePanel() {
  createPanel();
  isVisible = !isVisible;
  panelEl.classList.toggle("visible", isVisible);

  if (isVisible) {
    loadInitialLogs();
    startPolling();
  } else {
    stopPolling();
  }
}

function appendLines(lines) {
  if (!contentEl || !lines.length) return;

  for (const { id, text } of lines) {
    if (id <= lastId) continue;
    lastId = id;

    const div = document.createElement("div");
    div.className = "log-line";

    // Color by source
    if (text.startsWith("[stderr]")) {
      div.classList.add("stderr");
    } else if (text.startsWith("[Launcher]")) {
      div.classList.add("launcher");
    } else {
      div.classList.add("stdout");
    }

    // Highlight ROCm/Torch lines
    if (
      text.includes("ROCm") ||
      text.includes("HIP") ||
      text.includes("Torch") ||
      text.includes("comgr") ||
      text.includes("amdgpu") ||
      text.includes("ERROR") ||
      text.includes("error")
    ) {
      div.classList.add("highlight");
    }

    div.textContent = text;
    contentEl.appendChild(div);
  }

  // Trim old lines
  while (contentEl.children.length > MAX_LINES) {
    contentEl.removeChild(contentEl.firstChild);
  }

  applyFilter();
  updateCount();

  if (autoScroll) {
    contentEl.scrollTop = contentEl.scrollHeight;
  }
}

function applyFilter() {
  if (!contentEl) return;
  const lines = contentEl.querySelectorAll(".log-line");
  for (const line of lines) {
    if (!filterText) {
      line.classList.remove("filtered-out");
    } else {
      const match = line.textContent.toLowerCase().includes(filterText);
      line.classList.toggle("filtered-out", !match);
    }
  }
}

function updateCount() {
  if (!contentEl) return;
  const countEl = panelEl.querySelector("#logs-count");
  const visible = contentEl.querySelectorAll(".log-line:not(.filtered-out)").length;
  const total = contentEl.querySelectorAll(".log-line").length;
  countEl.textContent = filterText ? `${visible}/${total} lines` : `${total} lines`;
}

// ── Data fetching ──────────────────────────────────────────────────────────

async function loadInitialLogs() {
  try {
    const resp = await fetch("/logs/latest?lines=300");
    const data = await resp.json();
    if (data.lines && data.lines.length) {
      appendLines(data.lines);
    }
  } catch (e) {
    console.warn("[Desktop Logs] Failed to load initial logs:", e);
  }
}

async function pollForNewLogs() {
  try {
    const resp = await fetch(`/logs/stream?after=${lastId}`);
    const data = await resp.json();
    if (data.lines && data.lines.length) {
      appendLines(data.lines);
    }
  } catch (e) {
    // Silently retry
  }
}

function startPolling() {
  if (pollTimer) return;
  pollTimer = setInterval(pollForNewLogs, POLL_MS);
}

function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

// ── Register with ComfyUI ──────────────────────────────────────────────────

app.registerExtension({
  name: "ComfyUI.DesktopLogs",
  async setup() {
    // Add toggle button to ComfyUI menu bar
    const menuBar = document.querySelector(".comfy-menu");
    if (menuBar) {
      const btn = document.createElement("button");
      btn.textContent = "Logs";
      btn.title = "Toggle backend log viewer";
      btn.style.cssText =
        "font-size:12px;padding:4px 8px;margin-left:4px;background:#7c3aed;color:white;border:none;border-radius:4px;cursor:pointer;";
      btn.addEventListener("click", togglePanel);
      menuBar.appendChild(btn);
    }

    console.log("[Desktop Logs] Extension registered. Click 'Logs' button to open panel.");
  },
});
