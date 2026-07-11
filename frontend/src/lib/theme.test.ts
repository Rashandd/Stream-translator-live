import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  applyTheme,
  getStoredTheme,
  persistTheme,
  resolveInitialTheme,
  themeFromSystemPreference,
  THEME_STORAGE_KEY,
  watchSystemTheme,
} from "@/src/lib/theme";

type FakeMediaQueryList = {
  matches: boolean;
  addEventListener: (type: "change", listener: (event: { matches: boolean }) => void) => void;
  removeEventListener: (type: "change", listener: (event: { matches: boolean }) => void) => void;
  emit: (matches: boolean) => void;
};

function createFakeMediaQueryList(initialMatches: boolean): FakeMediaQueryList {
  let listeners: Array<(event: { matches: boolean }) => void> = [];
  const mql: FakeMediaQueryList = {
    matches: initialMatches,
    addEventListener: (_type, listener) => {
      listeners.push(listener);
    },
    removeEventListener: (_type, listener) => {
      listeners = listeners.filter((l) => l !== listener);
    },
    emit: (matches) => {
      mql.matches = matches;
      for (const listener of listeners) listener({ matches });
    },
  };
  return mql;
}

function stubDom(options: { storedTheme?: string | null; systemPrefersDark?: boolean } = {}) {
  const store = new Map<string, string>();
  if (options.storedTheme) store.set(THEME_STORAGE_KEY, options.storedTheme);

  const mql = createFakeMediaQueryList(options.systemPrefersDark ?? false);
  const classList = new Set<string>();

  vi.stubGlobal("window", {
    localStorage: {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => store.set(key, value),
    },
    matchMedia: (_query: string) => mql,
  });
  vi.stubGlobal("document", {
    documentElement: {
      classList: {
        toggle: (name: string, force: boolean) => {
          if (force) classList.add(name);
          else classList.delete(name);
        },
        add: (name: string) => classList.add(name),
      },
    },
  });

  return { mql, classList, store };
}

describe("theme", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  describe("getStoredTheme", () => {
    it("returns null when nothing is stored", () => {
      stubDom();
      expect(getStoredTheme()).toBeNull();
    });

    it("returns the stored theme when valid", () => {
      stubDom({ storedTheme: "dark" });
      expect(getStoredTheme()).toBe("dark");
    });

    it("ignores garbage values", () => {
      const { store } = stubDom();
      store.set(THEME_STORAGE_KEY, "purple");
      expect(getStoredTheme()).toBeNull();
    });
  });

  describe("themeFromSystemPreference", () => {
    it("returns dark when the OS prefers dark", () => {
      stubDom({ systemPrefersDark: true });
      expect(themeFromSystemPreference()).toBe("dark");
    });

    it("returns light when the OS prefers light", () => {
      stubDom({ systemPrefersDark: false });
      expect(themeFromSystemPreference()).toBe("light");
    });
  });

  describe("resolveInitialTheme", () => {
    it("prefers an explicit stored choice over the system preference", () => {
      stubDom({ storedTheme: "light", systemPrefersDark: true });
      expect(resolveInitialTheme()).toBe("light");
    });

    it("falls back to the system preference when nothing is stored", () => {
      stubDom({ systemPrefersDark: true });
      expect(resolveInitialTheme()).toBe("dark");
    });
  });

  describe("applyTheme / persistTheme", () => {
    it("toggles the dark class on the root element", () => {
      const { classList } = stubDom();
      applyTheme("dark");
      expect(classList.has("dark")).toBe(true);
      applyTheme("light");
      expect(classList.has("dark")).toBe(false);
    });

    it("persists the choice to storage", () => {
      const { store } = stubDom();
      persistTheme("dark");
      expect(store.get(THEME_STORAGE_KEY)).toBe("dark");
    });
  });

  describe("watchSystemTheme", () => {
    it("notifies on OS theme changes when the user has no stored preference", () => {
      const { mql } = stubDom({ systemPrefersDark: false });
      const onChange = vi.fn();
      watchSystemTheme(onChange);

      mql.emit(true);

      expect(onChange).toHaveBeenCalledWith("dark");
    });

    it("stays silent once the user has picked an explicit theme", () => {
      const { mql } = stubDom({ storedTheme: "light", systemPrefersDark: false });
      const onChange = vi.fn();
      watchSystemTheme(onChange);

      mql.emit(true);

      expect(onChange).not.toHaveBeenCalled();
    });

    it("returns an unsubscribe function that stops future notifications", () => {
      const { mql } = stubDom({ systemPrefersDark: false });
      const onChange = vi.fn();
      const unsubscribe = watchSystemTheme(onChange);

      unsubscribe();
      mql.emit(true);

      expect(onChange).not.toHaveBeenCalled();
    });
  });
});
