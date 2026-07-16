// StreamDubber Live Captions — broadcaster configuration page.
//
// The broadcaster links their Twitch channel to a StreamDubber account with a
// one-time 6-digit code generated in the StreamDubber dashboard. The claim is
// authorized by the Twitch extension JWT (role=broadcaster) on the EBS; no
// StreamDubber credentials are ever entered inside Twitch. Only public,
// non-secret values (configured flag, default language) are written to the
// Twitch Configuration Service.

import {
  DASHBOARD_URL,
  bindUrl,
  isValidLang,
  resolveEbsBase,
  stateUrl,
} from "./shared.js";

/** Offered defaults only — viewers always see the languages actually streamed. */
const DEFAULT_LANG_CHOICES = ["en", "tr", "de", "es", "fr", "pt", "ru", "ja"];

const state = {
  ebsBase: resolveEbsBase(),
  channelId: null,
  token: null,
  linked: false,
  defaultLang: "en",
};

const el = {
  form: document.getElementById("sd-link-form"),
  code: document.getElementById("sd-code"),
  submit: document.getElementById("sd-link-submit"),
  status: document.getElementById("sd-link-status"),
  statusText: document.getElementById("sd-link-status-text"),
  lang: document.getElementById("sd-default-lang"),
  save: document.getElementById("sd-save-settings"),
  settingsStatus: document.getElementById("sd-settings-status"),
  dashboard: document.getElementById("sd-dashboard-link"),
};

function setStatus(node, kind, text) {
  node.dataset.kind = kind;
  node.querySelector("span[data-text]").textContent = text;
}

function renderLinkState() {
  if (state.linked) {
    setStatus(el.status, "ok", "Channel linked to a StreamDubber account");
    el.submit.disabled = true;
    el.code.disabled = true;
  } else {
    setStatus(el.status, "pending", "Not linked yet");
    el.submit.disabled = false;
    el.code.disabled = false;
  }
}

function populateLanguages() {
  for (const code of DEFAULT_LANG_CHOICES) {
    const option = document.createElement("option");
    option.value = code;
    option.textContent = code.toUpperCase();
    el.lang.appendChild(option);
  }
  el.lang.value = state.defaultLang;
}

/** Persist public settings to the Twitch Configuration Service. */
function saveTwitchConfig() {
  const content = JSON.stringify({
    configured: state.linked,
    default_lang: state.defaultLang,
  });
  window.Twitch.ext.configuration.set("broadcaster", "1", content);
}

function readTwitchConfig() {
  try {
    const segment = window.Twitch.ext.configuration.broadcaster;
    if (!segment || !segment.content) {
      return;
    }
    const parsed = JSON.parse(segment.content);
    if (isValidLang(parsed.default_lang)) {
      state.defaultLang = parsed.default_lang.toLowerCase();
      el.lang.value = state.defaultLang;
    }
  } catch {
    // Broken config content falls back to defaults.
  }
}

/** The EBS is the source of truth for "is this channel linked". */
async function fetchLinkState() {
  if (!state.channelId || !state.token) {
    return;
  }
  try {
    const resp = await fetch(stateUrl(state.ebsBase, state.channelId), {
      headers: { Authorization: `Bearer ${state.token}` },
    });
    if (!resp.ok) {
      if (resp.status === 503) {
        setStatus(el.status, "error", "Captions service is not enabled yet");
      }
      return;
    }
    const body = await resp.json();
    state.linked = body.configured === true;
    renderLinkState();
  } catch {
    setStatus(el.status, "error", "Could not reach the captions service");
  }
}

async function submitCode(event) {
  event.preventDefault();
  const code = el.code.value.trim();
  if (!/^\d{6}$/.test(code)) {
    setStatus(el.status, "error", "The code is 6 digits — check the dashboard");
    return;
  }
  el.submit.disabled = true;
  setStatus(el.status, "pending", "Linking…");
  try {
    const resp = await fetch(bindUrl(state.ebsBase), {
      method: "POST",
      headers: {
        Authorization: `Bearer ${state.token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ code }),
    });
    if (resp.ok) {
      state.linked = true;
      renderLinkState();
      saveTwitchConfig();
      return;
    }
    const messages = {
      404: "Unknown or already used code — generate a fresh one",
      410: "That code expired — generate a fresh one",
      403: "Only the channel broadcaster can link",
      503: "Captions service is not enabled yet",
    };
    setStatus(
      el.status,
      "error",
      messages[resp.status] || "Linking failed — please try again",
    );
    el.submit.disabled = false;
  } catch {
    setStatus(el.status, "error", "Could not reach the captions service");
    el.submit.disabled = false;
  }
}

el.form.addEventListener("submit", submitCode);

el.save.addEventListener("click", () => {
  const value = el.lang.value;
  if (isValidLang(value)) {
    state.defaultLang = value.toLowerCase();
    saveTwitchConfig();
    setStatus(el.settingsStatus, "ok", "Saved");
  }
});

el.dashboard.href = DASHBOARD_URL;

populateLanguages();
renderLinkState();

if (window.Twitch && window.Twitch.ext) {
  window.Twitch.ext.onAuthorized((auth) => {
    const firstAuth = state.token === null;
    state.channelId = auth.channelId;
    state.token = auth.token;
    if (firstAuth) {
      readTwitchConfig();
      fetchLinkState();
    }
  });
  window.Twitch.ext.configuration.onChanged(readTwitchConfig);
}
