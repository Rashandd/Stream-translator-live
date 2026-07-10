export const THEME_STORAGE_KEY = "streamdubber-theme";

export type Theme = "light" | "dark";

/**
 * No stored preference yet: default to dark outside local daytime hours
 * (07:00-19:00), matching the user's device clock.
 */
export function themeForLocalHour(hour: number): Theme {
  return hour >= 7 && hour < 19 ? "light" : "dark";
}

export function getStoredTheme(): Theme | null {
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
  return stored === "light" || stored === "dark" ? stored : null;
}

export function resolveInitialTheme(): Theme {
  return getStoredTheme() ?? themeForLocalHour(new Date().getHours());
}

export function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

export function persistTheme(theme: Theme) {
  window.localStorage.setItem(THEME_STORAGE_KEY, theme);
}
