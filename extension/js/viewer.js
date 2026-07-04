// StreamDubber Live Captions — video overlay viewer.
//
// Data flow: Twitch helper `onAuthorized` supplies the channel id and a
// short-lived JWT → the caption state is fetched from the EBS → while the
// channel is live a WebSocket streams subtitle frames. If the WebSocket
// cannot be established repeatedly, the viewer degrades to polling the state
// endpoint. All dynamic text is rendered with `textContent`; nothing from the
// network is ever interpreted as HTML.

import {
  POLL_INTERVAL_MS,
  WS_FAILURES_BEFORE_POLL,
  captionTextFor,
  captionsWsUrl,
  clampSize,
  computeStatus,
  cycleSize,
  isValidLang,
  languagesOf,
  loadPrefs,
  nextDelayMs,
  parseSubtitle,
  pickLanguage,
  resolveEbsBase,
  sanitizeLanguages,
  savePrefs,
  stateUrl,
} from "./shared.js";

const STATUS_LABELS = {
  not_configured: "Captions not configured",
  offline: "Channel offline",
  connecting: "Connecting",
  waiting: "Waiting for captions",
  live: "Live",
  delayed: "Captions delayed",
};

const state = {
  ebsBase: resolveEbsBase(window.location.hostname, window.location.search),
  channelId: null,
  token: null,
  configured: false,
  live: false,
  connected: false,
  lastCaptionAt: null,
  languages: [],
  defaultLang: null,
  currentSubtitle: null,
  prefs: loadPrefs(window.localStorage),
  ws: null,
  wsFailures: 0,
  pollTimer: null,
  reconnectTimer: null,
  statusTimer: null,
};

const el = {
  root: document.getElementById("sd-root"),
  caption: document.getElementById("sd-caption"),
  captionText: document.getElementById("sd-caption-text"),
  bar: document.getElementById("sd-bar"),
  status: document.getElementById("sd-status"),
  statusDot: document.getElementById("sd-status-dot"),
  statusLabel: document.getElementById("sd-status-label"),
  lang: document.getElementById("sd-lang"),
  size: document.getElementById("sd-size"),
  toggle: document.getElementById("sd-toggle"),
  toggleOn: document.getElementById("sd-toggle-on"),
  toggleOff: document.getElementById("sd-toggle-off"),
};

// --- Rendering (textContent only) ------------------------------------------

function render() {
  const status = computeStatus({
    configured: state.configured,
    live: state.live,
    connected: state.connected,
    lastCaptionAgeMs: state.lastCaptionAt === null ? null : Date.now() - state.lastCaptionAt,
  });

  el.statusLabel.textContent = STATUS_LABELS[status];
  el.statusDot.dataset.status = status;
  el.root.dataset.size = clampSize(state.prefs.size);
  el.root.dataset.visible = state.prefs.visible ? "on" : "off";
  el.toggleOn.hidden = !state.prefs.visible;
  el.toggleOff.hidden = state.prefs.visible;

  const lang = state.prefs.lang;
  let text = "";
  if (state.prefs.visible && state.currentSubtitle && lang) {
    const candidate = captionTextFor(state.currentSubtitle, lang);
    if (candidate !== null) {
      text = candidate;
    }
  }
  el.captionText.textContent = text;
  el.caption.hidden = text.length === 0;
}

function renderLanguageOptions() {
  while (el.lang.firstChild) {
    el.lang.removeChild(el.lang.firstChild);
  }
  for (const code of state.languages) {
    const option = document.createElement("option");
    option.value = code;
    option.textContent = code.toUpperCase();
    el.lang.appendChild(option);
  }
  const picked = pickLanguage(state.languages, state.prefs.lang, state.defaultLang);
  if (picked) {
    state.prefs.lang = picked;
    el.lang.value = picked;
  }
  el.lang.disabled = state.languages.length === 0;
}

function absorbLanguages(langs) {
  const merged = sanitizeLanguages([...state.languages, ...langs]);
  if (merged.join(",") !== state.languages.join(",")) {
    state.languages = merged;
    renderLanguageOptions();
  }
}

function absorbSubtitle(subtitle) {
  state.currentSubtitle = subtitle;
  state.lastCaptionAt = Date.now();
  absorbLanguages(languagesOf(subtitle));
  render();
}

// --- EBS state fetch (initial load + poll fallback) -------------------------

async function fetchState() {
  if (!state.channelId || !state.token) {
    return;
  }
  try {
    const resp = await fetch(stateUrl(state.ebsBase, state.channelId), {
      headers: { Authorization: `Bearer ${state.token}` },
    });
    if (!resp.ok) {
      return;
    }
    const body = await resp.json();
    state.configured = body.configured === true;
    state.live = body.live === true;
    state.defaultLang = isValidLang(body.default_lang) ? body.default_lang.toLowerCase() : null;
    absorbLanguages(sanitizeLanguages(body.languages));
    if (Array.isArray(body.lines)) {
      for (const line of body.lines) {
        const subtitle = parseSubtitle(line);
        if (subtitle) {
          absorbSubtitle(subtitle);
        }
      }
    }
    if (state.live && !state.connected && !state.reconnectTimer) {
      connectWs();
    }
    render();
  } catch {
    // Network hiccups: the status stays "connecting"/"offline"; next poll retries.
  }
}

function startPolling() {
  if (state.pollTimer) {
    return;
  }
  state.pollTimer = window.setInterval(fetchState, POLL_INTERVAL_MS);
}

function stopPolling() {
  if (state.pollTimer) {
    window.clearInterval(state.pollTimer);
    state.pollTimer = null;
  }
}

// --- Live captions WebSocket -------------------------------------------------

function connectWs() {
  if (!state.channelId || !state.token || state.ws) {
    return;
  }
  let socket;
  try {
    socket = new WebSocket(captionsWsUrl(state.ebsBase, state.channelId, state.token));
  } catch {
    scheduleReconnect();
    return;
  }
  state.ws = socket;

  socket.onopen = () => {
    state.connected = true;
    state.wsFailures = 0;
    stopPolling();
    render();
  };
  socket.onmessage = (event) => {
    const subtitle = parseSubtitle(event.data);
    if (subtitle) {
      absorbSubtitle(subtitle);
    }
  };
  socket.onclose = () => {
    state.ws = null;
    state.connected = false;
    state.wsFailures += 1;
    render();
    scheduleReconnect();
  };
  socket.onerror = () => {
    socket.close();
  };
}

function scheduleReconnect() {
  if (state.reconnectTimer) {
    return;
  }
  if (state.wsFailures >= WS_FAILURES_BEFORE_POLL) {
    // Transport gate fallback (policy): keep captions flowing via polling,
    // retry the socket occasionally.
    startPolling();
  }
  const delay = nextDelayMs(state.wsFailures);
  state.reconnectTimer = window.setTimeout(() => {
    state.reconnectTimer = null;
    fetchState().then(() => {
      if (state.live) {
        connectWs();
      }
    });
  }, delay);
}

// --- Controls ----------------------------------------------------------------

el.lang.addEventListener("change", () => {
  const value = el.lang.value;
  if (isValidLang(value)) {
    state.prefs.lang = value.toLowerCase();
    savePrefs(window.localStorage, state.prefs);
    render();
  }
});

el.size.addEventListener("click", () => {
  state.prefs.size = cycleSize(state.prefs.size);
  savePrefs(window.localStorage, state.prefs);
  render();
});

el.toggle.addEventListener("click", () => {
  state.prefs.visible = !state.prefs.visible;
  savePrefs(window.localStorage, state.prefs);
  render();
});

// --- Twitch helper wiring ------------------------------------------------------

function readBroadcasterConfig() {
  try {
    const segment = window.Twitch.ext.configuration.broadcaster;
    if (!segment || !segment.content) {
      return;
    }
    const parsed = JSON.parse(segment.content);
    if (isValidLang(parsed.default_lang)) {
      state.defaultLang = parsed.default_lang.toLowerCase();
    }
  } catch {
    // Missing/invalid configuration is equivalent to "no default".
  }
}

if (window.Twitch && window.Twitch.ext) {
  window.Twitch.ext.onAuthorized((auth) => {
    const firstAuth = state.token === null;
    state.channelId = auth.channelId;
    // Twitch refreshes the token periodically; later fetches/reconnects use
    // the newest one automatically.
    state.token = auth.token;
    if (firstAuth) {
      readBroadcasterConfig();
      fetchState();
      // Re-evaluate "delayed" while captions are idle.
      state.statusTimer = window.setInterval(render, 3000);
    }
  });
  window.Twitch.ext.configuration.onChanged(readBroadcasterConfig);
} else {
  // Outside Twitch (direct file open) the overlay stays inert by design.
  el.statusLabel.textContent = STATUS_LABELS.not_configured;
}

render();
