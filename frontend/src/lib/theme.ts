export const THEME_STORAGE_KEY = "streamdubber-theme";

const DARK_MEDIA_QUERY = "(prefers-color-scheme: dark)";

export type Theme = "light" | "dark";

/** No stored preference yet: default to whatever the OS/browser reports. */
export function themeFromSystemPreference(): Theme {
  return window.matchMedia(DARK_MEDIA_QUERY).matches ? "dark" : "light";
}

export function getStoredTheme(): Theme | null {
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
  return stored === "light" || stored === "dark" ? stored : null;
}

export function resolveInitialTheme(): Theme {
  return getStoredTheme() ?? themeFromSystemPreference();
}

export function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

export function persistTheme(theme: Theme) {
  window.localStorage.setItem(THEME_STORAGE_KEY, theme);
}

/**
 * Follows the OS theme live as long as the user hasn't made an explicit
 * choice (persistTheme wasn't called yet). Returns an unsubscribe function.
 */
export function watchSystemTheme(onChange: (theme: Theme) => void): () => void {
  const media = window.matchMedia(DARK_MEDIA_QUERY);
  const listener = (event: { matches: boolean }) => {
    if (getStoredTheme() !== null) return;
    onChange(event.matches ? "dark" : "light");
  };
  media.addEventListener("change", listener);
  return () => media.removeEventListener("change", listener);
}
