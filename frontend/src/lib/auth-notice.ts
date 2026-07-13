export type AuthNoticeCode =
  | "sign_in_required"
  | "session_expired"
  | "session_unavailable";

const NOTICE_KEY = "sd.auth.notice";
const SESSION_SEEN_KEY = "sd.auth.session-seen";
export const LOGOUT_EVENT_KEY = "sd.auth.logout";
export const AUTH_CHANNEL = "sd.auth";

export function storeAuthNotice(code: AuthNoticeCode): void {
  window.sessionStorage.setItem(NOTICE_KEY, code);
}

export function consumeAuthNotice(): AuthNoticeCode | null {
  const value = window.sessionStorage.getItem(NOTICE_KEY);
  window.sessionStorage.removeItem(NOTICE_KEY);
  return value === "sign_in_required" ||
    value === "session_expired" ||
    value === "session_unavailable"
    ? value
    : null;
}

export function hasSeenSession(): boolean {
  return window.localStorage.getItem(SESSION_SEEN_KEY) === "1";
}

export function markSessionSeen(): void {
  window.localStorage.setItem(SESSION_SEEN_KEY, "1");
  // One-release migration cleanup: browser auth no longer uses this bearer.
  window.localStorage.removeItem("sd.user.token");
}

export function publishLogout(): void {
  window.localStorage.removeItem(SESSION_SEEN_KEY);
  window.localStorage.removeItem("sd.user.token");
  window.localStorage.setItem(LOGOUT_EVENT_KEY, String(Date.now()));
  if (typeof BroadcastChannel !== "undefined") {
    const channel = new BroadcastChannel(AUTH_CHANNEL);
    channel.postMessage("logout");
    channel.close();
  }
}
