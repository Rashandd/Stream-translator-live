"use client";

import Link from "next/link";
import { useState } from "react";
import { Icon } from "@/src/components/icon";
import { ThemeToggle } from "@/src/components/theme-toggle";
import { localizePath, type Locale } from "@/src/i18n/routing";

type MenuLink = { label: string; href: string };

type MobileMenuProps = {
  ariaLabel: string;
  menuLabel: string;
  menuCloseLabel: string;
  items: MenuLink[];
  signIn: MenuLink;
  locale: Locale;
};

export function MobileMenu({ ariaLabel, menuLabel, menuCloseLabel, items, signIn, locale }: MobileMenuProps) {
  const [open, setOpen] = useState(false);

  return (
    <div className="md:hidden">
      <button
        aria-expanded={open}
        aria-label={open ? menuCloseLabel : menuLabel}
        className="inline-flex h-11 w-11 items-center justify-center rounded-full border border-border bg-surface text-ink"
        onClick={() => setOpen((v) => !v)}
        type="button"
      >
        <Icon aria-hidden className="h-5 w-5" name={open ? "X" : "Menu"} />
      </button>

      {open ? (
        <nav
          aria-label={ariaLabel}
          className="absolute inset-x-0 top-full z-50 border-b border-border bg-surface px-4 pb-5 pt-2 shadow-panel"
        >
          <ul className="space-y-1">
            {items.map((item) => (
              <li key={item.href}>
                <Link
                  className="flex min-h-11 items-center rounded-lg px-3 py-2.5 text-base font-semibold text-text transition hover:bg-canvas hover:text-ink"
                  href={localizePath(locale, item.href)}
                  onClick={() => setOpen(false)}
                >
                  {item.label}
                </Link>
              </li>
            ))}
          </ul>
          <div className="mt-3 flex items-center gap-2 border-t border-hairline pt-4">
            <Link
              className="inline-flex h-11 flex-1 items-center justify-center rounded-full border border-border bg-surface px-5 text-sm font-semibold text-ink"
              href={localizePath(locale, signIn.href)}
              onClick={() => setOpen(false)}
            >
              {signIn.label}
            </Link>
            <ThemeToggle />
          </div>
        </nav>
      ) : null}
    </div>
  );
}
