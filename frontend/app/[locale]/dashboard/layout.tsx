import Link from "next/link";
import type { ReactNode } from "react";
import { notFound } from "next/navigation";
import { AuthGuard } from "@/src/components/auth-guard";
import { BrandLogo } from "@/src/components/brand-logo";
import { DashboardNav } from "@/src/components/dashboard-nav";
import { Icon } from "@/src/components/icon";
import { SignOutLink } from "@/src/components/sign-out-link";
import { StatusStrip } from "@/src/components/status-strip";
import { ThemeToggle } from "@/src/components/theme-toggle";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type DashboardLayoutProps = {
  children: ReactNode;
  params: Promise<{ locale: string }>;
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function DashboardLayout({ children, params }: DashboardLayoutProps) {
  const { locale: rawLocale } = await params;

  if (!isLocale(rawLocale)) {
    notFound();
  }

  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { dashboard } = config;

  return (
    <div className="min-h-screen bg-canvas">
      <header className="border-b border-hairline bg-surface">
        <div className="mx-auto flex w-full max-w-7xl items-center justify-between px-4 py-3 sm:px-6 lg:px-8">
          <Link className="flex shrink-0 items-center" href={localizePath(locale, "/")}>
            <BrandLogo
              className="h-12 w-auto object-contain sm:h-14"
              config={config}
              height={545}
              priority
              width={2253}
            />
          </Link>
          <div className="flex items-center gap-2 sm:gap-3">
            <Link
              className="hidden rounded-full border border-border px-3 py-1.5 text-xs font-semibold text-muted transition hover:text-ink sm:inline-flex"
              href={localizePath(locale, "/")}
            >
              {dashboard.topbar.backToSiteLabel}
            </Link>
            <span className="inline-flex h-9 w-9 items-center justify-center rounded-full bg-canvas text-ink">
              <Icon className="h-[18px] w-[18px]" name="Settings" />
            </span>
            <ThemeToggle />
            <SignOutLink
              className="inline-flex h-9 items-center justify-center rounded-full bg-primary px-4 text-xs font-semibold text-on-primary transition hover:bg-primary-hover"
              href={localizePath(locale, "/login")}
              label={dashboard.topbar.signOutLabel}
              tokenKind="user"
            />
          </div>
        </div>
      </header>

      <AuthGuard loginHref={localizePath(locale, "/login")} tokenKind="user">
        <div className="mx-auto w-full max-w-7xl px-4 sm:px-6 lg:flex lg:gap-8 lg:px-8">
          <aside className="border-b border-hairline lg:border-b-0">
            <DashboardNav
              ariaLabel={dashboard.nav.ariaLabel}
              groups={dashboard.nav.groups}
              locale={locale}
              statusLabels={config.statusLabels}
            />
          </aside>

          <main className="min-w-0 flex-1 py-6 lg:py-8">
            <div className="mb-6">
              <StatusStrip ariaLabel={dashboard.statusStrip.ariaLabel} items={dashboard.statusStrip.items} />
            </div>
            {children}
          </main>
        </div>
      </AuthGuard>
    </div>
  );
}
