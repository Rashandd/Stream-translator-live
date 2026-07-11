"use client";

import { useEffect, useState } from "react";
import { Icon } from "@/src/components/icon";
import { applyTheme, persistTheme, resolveInitialTheme, watchSystemTheme, type Theme } from "@/src/lib/theme";

type ThemeToggleProps = {
  lightLabel?: string;
  darkLabel?: string;
};

export function ThemeToggle({ lightLabel = "Switch to light theme", darkLabel = "Switch to dark theme" }: ThemeToggleProps) {
  const [theme, setTheme] = useState<Theme | null>(null);

  useEffect(() => {
    setTheme(resolveInitialTheme());
    return watchSystemTheme((next) => {
      setTheme(next);
      applyTheme(next);
    });
  }, []);

  if (theme === null) {
    return <span aria-hidden className="inline-flex h-9 w-9 shrink-0 rounded-full border border-border bg-surface" />;
  }

  const toggle = () => {
    const next: Theme = theme === "dark" ? "light" : "dark";
    setTheme(next);
    applyTheme(next);
    persistTheme(next);
  };

  const label = theme === "dark" ? lightLabel : darkLabel;

  return (
    <button
      aria-label={label}
      className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-border bg-surface text-ink transition hover:border-cyan hover:bg-cyan/10"
      onClick={toggle}
      title={label}
      type="button"
    >
      <Icon aria-hidden className="h-[18px] w-[18px]" name={theme === "dark" ? "Sun" : "Moon"} />
    </button>
  );
}
