import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  consumeAuthNotice,
  hasSeenSession,
  markSessionSeen,
  publishLogout,
  storeAuthNotice,
} from "@/src/lib/auth-notice";

class MemoryStorage {
  private values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
  clear() { this.values.clear(); }
}

describe("auth notice state", () => {
  const localStorage = new MemoryStorage();
  const sessionStorage = new MemoryStorage();

  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    vi.stubGlobal("window", { localStorage, sessionStorage });
    vi.stubGlobal("BroadcastChannel", undefined);
  });

  it("consumes a notice exactly once", () => {
    storeAuthNotice("session_expired");
    expect(consumeAuthNotice()).toBe("session_expired");
    expect(consumeAuthNotice()).toBeNull();
  });

  it("tracks successful sessions and clears the legacy bearer", () => {
    localStorage.setItem("sd.user.token", "stale");
    markSessionSeen();
    expect(hasSeenSession()).toBe(true);
    expect(localStorage.getItem("sd.user.token")).toBeNull();
  });

  it("publishes logout state and clears the session marker", () => {
    markSessionSeen();
    publishLogout();
    expect(hasSeenSession()).toBe(false);
    expect(localStorage.getItem("sd.auth.logout")).not.toBeNull();
  });
});
