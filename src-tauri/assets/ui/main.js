// Get DOM elements
const statusTextEl = document.getElementById("status-text");
const progressFillEl = document.getElementById("progress-fill");
const launcherContainer = document.getElementById("launcher-container");
const logsPanel = document.getElementById("logs-panel");
const logsHeader = document.getElementById("logs-header");
const logsViewport = document.getElementById("logs-viewport");
const logsContent = document.getElementById("logs-content");
const btnCopy = document.getElementById("btn-copy");
const btnToggle = document.getElementById("btn-toggle");
const downloadActions = document.getElementById("download-actions");
const btnCancel = document.getElementById("btn-cancel");
const btnRetry = document.getElementById("btn-retry");

let isFailed = false;
let isDownloading = false;
let lastLogId = null;

function setStatus(text, progress) {
  if (statusTextEl) statusTextEl.textContent = text;
  if (progressFillEl && progress != null) {
    progressFillEl.style.width = `${Math.min(Math.max(progress, 0), 100)}%`;
  }
}

function setFailed(message) {
  if (isFailed) return;
  isFailed = true;
  launcherContainer.classList.add("failed");
  setStatus("Error: Failed to load ComfyUI!");
  logsPanel.classList.remove("collapsed");
  launcherContainer.classList.add("expanded");
  if (downloadActions) downloadActions.classList.remove("visible");
  appendLogLine(`[FATAL ERROR] ${message}`, "error");
}

function appendLogLine(text, type) {
  const line = document.createElement("div");
  line.className = `log-line ${type}`;
  line.textContent = text;
  logsContent.appendChild(line);
  logsViewport.scrollTop = logsViewport.scrollHeight;
}

function toggleLogs() {
  if (logsPanel.classList.contains("collapsed")) {
    logsPanel.classList.remove("collapsed");
    launcherContainer.classList.add("expanded");
    setTimeout(() => {
      logsViewport.scrollTop = logsViewport.scrollHeight;
    }, 100);
  } else {
    logsPanel.classList.add("collapsed");
    launcherContainer.classList.remove("expanded");
  }
}

logsHeader.addEventListener("click", (e) => {
  if (e.target.closest("#btn-copy")) return;
  toggleLogs();
});

btnToggle.addEventListener("click", (e) => {
  e.stopPropagation();
  toggleLogs();
});

btnCopy.addEventListener("click", async (e) => {
  e.stopPropagation();

  try {
    let logsArray = [];
    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        const res = await window.__TAURI__.core.invoke("get_logs");
        const data = Array.isArray(res) ? res[0] : res;
        logsArray = Array.isArray(data)
          ? data.map(entry => Array.isArray(entry) ? entry[1] : entry)
          : [];
      } catch (invokeErr) {
        console.error("Invoke get_logs failed:", invokeErr);
        const lines = logsContent.querySelectorAll(".log-line");
        logsArray = Array.from(lines).map(line => line.textContent);
      }
    } else {
      const lines = logsContent.querySelectorAll(".log-line");
      logsArray = Array.from(lines).map(line => line.textContent);
    }

    const fullLogs = logsArray.join("\n");
    await navigator.clipboard.writeText(fullLogs);

    btnCopy.classList.add("success");
    btnCopy.innerHTML = `
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12"></polyline>
      </svg>
      Logs Copied!
    `;

    setTimeout(() => {
      btnCopy.classList.remove("success");
      btnCopy.innerHTML = `
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
        </svg>
        Copy Logs
      `;
    }, 2000);
  } catch (err) {
    console.error("Failed to copy logs:", err);
  }
});

if (btnCancel) {
  btnCancel.addEventListener("click", async () => {
    try {
      await window.__TAURI__.core.invoke("cancel_backend_download");
      setStatus("Download cancelled.");
      appendLogLine("[Launcher] Download cancelled by user.", "system");
      if (downloadActions) downloadActions.classList.remove("visible");
      isDownloading = false;
    } catch (err) {
      console.error("Cancel failed:", err);
    }
  });
}

if (btnRetry) {
  btnRetry.addEventListener("click", async () => {
    isFailed = false;
    launcherContainer.classList.remove("failed");
    setStatus("Starting download...", 0);
    if (downloadActions) {
      btnRetry.classList.remove("visible");
      btnCancel.classList.add("visible");
    }
    try {
      await window.__TAURI__.core.invoke("start_backend_download");
      isDownloading = true;
      appendLogLine("[Launcher] Download started...", "system");
    } catch (err) {
      setFailed(`Failed to start download: ${err}`);
    }
  });
}

function handleLogEvent(event, type) {
  const payload = event.payload;
  if (Array.isArray(payload)) {
    payload.forEach(msg => appendLogLine(msg, type));
  } else if (typeof payload === 'string') {
    appendLogLine(payload, type);
  }
}

async function checkAndStart() {
  try {
    const status = await window.__TAURI__.core.invoke("check_backend_status");
    const isInstalled = status[0];
    const version = status[1];

    if (isInstalled) {
      setStatus(`Backend v${version || '?'} ready. Starting ComfyUI...`, 50);
      appendLogLine(`[Launcher] Backend installed (v${version || '?'}). Starting ComfyUI...`, "system");
      await window.__TAURI__.core.invoke("start_comfyui");
    } else {
      setStatus("Backend not found. Starting download...", 5);
      appendLogLine("[Launcher] Backend not installed. Starting download...", "system");
      if (downloadActions) downloadActions.classList.add("visible");
      if (btnCancel) btnCancel.classList.add("visible");
      isDownloading = true;
      await window.__TAURI__.core.invoke("start_backend_download");
    }
  } catch (err) {
    setFailed(`Failed to initialize: ${err}`);
  }
}

if (window.__TAURI__) {
  try {
    const tauriEvent = window.__TAURI__.event;

    if (tauriEvent && typeof tauriEvent.listen === 'function') {
      tauriEvent.listen("comfyui-log", (event) => {
        handleLogEvent(event, "system");
      }).catch(err => console.error("Error setting up comfyui-log listener:", err));

      tauriEvent.listen("comfyui-log-stdout", (event) => {
        handleLogEvent(event, "stdout");
      }).catch(err => console.error("Error setting up comfyui-log-stdout listener:", err));

      tauriEvent.listen("comfyui-log-stderr", (event) => {
        handleLogEvent(event, "stderr");
      }).catch(err => console.error("Error setting up comfyui-log-stderr listener:", err));

      tauriEvent.listen("comfyui-error", (event) => {
        setFailed(event.payload);
      }).catch(err => console.error("Error setting up comfyui-error listener:", err));

      tauriEvent.listen("comfyui-timeout", (event) => {
        setFailed(event.payload);
      }).catch(err => console.error("Error setting up comfyui-timeout listener:", err));

      tauriEvent.listen("comfyui-download-start", () => {
        setStatus("Downloading backend dependencies...", 5);
        appendLogLine("[Launcher] Backend not found. Starting download...", "system");
      }).catch(err => console.error("Error setting up download-start listener:", err));

      tauriEvent.listen("comfyui-download-progress", (event) => {
        const p = event.payload;
        if (p && typeof p === 'object') {
          const pct = p.total_bytes > 0 ? Math.round((p.bytes_downloaded / p.total_bytes) * 100) : 0;
          setStatus(p.message || `Downloading... ${pct}%`, Math.min(pct, 99));
          if (p.message) appendLogLine(`[Launcher] ${p.message}`, "system");
        }
      }).catch(err => console.error("Error setting up download-progress listener:", err));

      tauriEvent.listen("comfyui-download-complete", () => {
        setStatus("Backend ready. Starting ComfyUI...", 100);
        appendLogLine("[Launcher] Backend download complete!", "system");
        if (downloadActions) downloadActions.classList.remove("visible");
        isDownloading = false;
        window.__TAURI__.core.invoke("start_comfyui").catch(err => {
          setFailed(`Failed to start ComfyUI: ${err}`);
        });
      }).catch(err => console.error("Error setting up download-complete listener:", err));

      tauriEvent.listen("comfyui-download-error", (event) => {
        setFailed(`Backend download failed: ${event.payload}`);
        if (downloadActions) {
          btnCancel.classList.remove("visible");
          btnRetry.classList.add("visible");
        }
        isDownloading = false;
      }).catch(err => console.error("Error setting up download-error listener:", err));
    } else {
      console.warn("window.__TAURI__.event is not fully initialized.");
      appendLogLine("[Warning] window.__TAURI__.event is not available.", "system");
    }
  } catch (e) {
    console.error("Tauri events setup failed:", e);
    appendLogLine(`[Error] Failed to initialize Tauri events: ${e.message}`, "error");
  }
} else {
  appendLogLine("[Dev Mode] Running outside Tauri webview container.", "system");
}

// Initial check
checkAndStart();
