const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const pairingView = document.getElementById("pairing-view");
const statusView = document.getElementById("status-view");
const pairForm = document.getElementById("pair-form");
const pairSubmit = document.getElementById("pair-submit");
const pairError = document.getElementById("pair-error");
const unpairBtn = document.getElementById("unpair-btn");
const updateBanner = document.getElementById("update-banner");
const updateMessage = document.getElementById("update-message");
const updateRestartBtn = document.getElementById("update-restart-btn");

function render(status) {
  if (status.paired) {
    pairingView.hidden = true;
    statusView.hidden = false;
    document.getElementById("status-backend").textContent = status.backend_url ?? "—";
    document.getElementById("status-relay").textContent = status.relay_listening
      ? "Listening for OBS"
      : "Starting…";
    document.getElementById("status-mic").textContent =
      status.mic_capture_ok === null
        ? "Checking…"
        : status.mic_capture_ok
          ? "Capturing"
          : "Unavailable — subtitles disabled";
    document.getElementById("status-targets").textContent = String(status.targets_synced ?? 0);
  } else {
    statusView.hidden = true;
    pairingView.hidden = false;
  }
}

async function refreshStatus() {
  const status = await invoke("get_status");
  render(status);
}

/** Renders the `update-status` events emitted by `main::check_for_update`. */
function renderUpdateBanner(status) {
  updateRestartBtn.hidden = true;
  updateBanner.classList.remove("error");
  switch (status.state) {
    case "checking":
      updateMessage.textContent = "Checking for updates…";
      updateBanner.hidden = false;
      break;
    case "up_to_date":
      updateBanner.hidden = true;
      break;
    case "available":
      updateMessage.textContent = `Update v${status.version} available, downloading…`;
      updateBanner.hidden = false;
      break;
    case "downloading":
      updateMessage.textContent = "Downloading update…";
      updateBanner.hidden = false;
      break;
    case "ready":
      updateMessage.textContent = "Update ready — restart to apply.";
      updateRestartBtn.hidden = false;
      updateBanner.hidden = false;
      break;
    case "failed":
      updateMessage.textContent = `Update check failed: ${status.message}`;
      updateBanner.classList.add("error");
      updateBanner.hidden = false;
      break;
    default:
      updateBanner.hidden = true;
  }
}

updateRestartBtn.addEventListener("click", async () => {
  updateRestartBtn.disabled = true;
  try {
    await invoke("restart_app");
  } finally {
    updateRestartBtn.disabled = false;
  }
});

listen("update-status", (event) => renderUpdateBanner(event.payload));

function showError(message) {
  pairError.textContent = message;
  pairError.hidden = false;
}

function guessDeviceName() {
  const platform = navigator.platform || navigator.userAgent || "Desktop";
  if (platform.startsWith("Win")) return "Windows PC";
  if (platform.startsWith("Mac")) return "Mac";
  if (platform.startsWith("Linux")) return "Linux PC";
  return "Desktop";
}

document.getElementById("device-name").value = guessDeviceName();

pairForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  pairError.hidden = true;
  pairSubmit.disabled = true;
  pairSubmit.textContent = "Pairing…";
  try {
    await invoke("submit_pairing_code", {
      backendUrl: document.getElementById("backend-url").value.trim(),
      code: document.getElementById("pair-code").value.trim(),
      deviceName: document.getElementById("device-name").value.trim() || "Desktop",
    });
    await refreshStatus();
  } catch (err) {
    showError(typeof err === "string" ? err : "Could not pair. Check the code and try again.");
  } finally {
    pairSubmit.disabled = false;
    pairSubmit.textContent = "Pair";
  }
});

unpairBtn.addEventListener("click", async () => {
  unpairBtn.disabled = true;
  try {
    await invoke("unpair");
    await refreshStatus();
  } finally {
    unpairBtn.disabled = false;
  }
});

listen("status-changed", (event) => render(event.payload));

refreshStatus();
