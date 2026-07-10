"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { StatusPill } from "@/src/components/status-pill";
import { localizePath, type Locale } from "@/src/i18n/routing";

type NavItem = {
  label: string;
  href: string;
  icon: string;
  status?: string;
};

type NavGroup = {
  label: string;
  items: NavItem[];
};

type DashboardNavProps = {
  ariaLabel: string;
  groups: NavGroup[];
  statusLabels: Record<string, string>;
  locale: Locale;
  rootHref?: string;
};

export function DashboardNav({ ariaLabel, groups, statusLabels, locale, rootHref = "/dashboard" }: DashboardNavProps) {
  const pathname = usePathname();

  const isActive = (href: string) => {
    const target = localizePath(locale, href);
    if (href === rootHref) {
      return pathname === target;
    }
    return pathname === target || pathname.startsWith(`${target}/`);
  };

  return (
    <nav
      aria-label={ariaLabel}
      className="flex gap-1 overflow-x-auto px-1 py-3 lg:w-60 lg:shrink-0 lg:flex-col lg:gap-6 lg:overflow-visible lg:px-0 lg:py-6"
    >
      {groups.map((group) => (
        <div className="flex shrink-0 items-center gap-1 lg:flex-col lg:items-stretch lg:gap-1" key={group.label}>
          <p className="hidden px-3 text-xs font-semibold uppercase tracking-wide text-faint lg:block">{group.label}</p>
          {group.items.map((item) => {
            const active = isActive(item.href);
            return (
              <Link
                aria-current={active ? "page" : undefined}
                className={`flex shrink-0 items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-semibold transition ${
                  active ? "bg-primary text-white" : "text-text hover:bg-surface hover:text-ink"
                }`}
                href={localizePath(locale, item.href)}
                key={item.href}
              >
                <Icon className={`h-[18px] w-[18px] ${active ? "text-white" : "text-muted"}`} name={item.icon} />
                <span className="whitespace-nowrap">{item.label}</span>
                {item.status ? (
                  <span className="hidden lg:ml-auto lg:block">
                    <StatusPill label={item.status} text={statusLabels[item.status] ?? item.status} />
                  </span>
                ) : null}
              </Link>
            );
          })}
        </div>
      ))}
    </nav>
  );
}
