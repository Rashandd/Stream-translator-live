import Image from "next/image";
import Link from "next/link";
import { MobileMenu } from "@/src/components/mobile-menu";
import type { PublicConfig } from "@/src/lib/config";
import { localizePath, type Locale } from "@/src/i18n/routing";

type SiteHeaderProps = {
  config: PublicConfig;
  locale: Locale;
};

export function SiteHeader({ config, locale }: SiteHeaderProps) {
  return (
    <header className="relative border-b border-hairline bg-white/86 backdrop-blur">
      <div className="flex w-full items-center justify-between px-4 py-3 sm:px-6 lg:px-8">
      <div className="flex min-w-0 items-center gap-10 lg:gap-16">
        <Link className="flex min-w-0 shrink-0 items-center" href={localizePath(locale, "/")}>
          <Image
            alt={config.brand.name}
            className="h-14 w-auto object-contain sm:h-20 lg:h-24"
            height={545}
            priority
            src={config.brand.logoAsset}
            width={2253}
          />
        </Link>

        <nav aria-label={config.nav.ariaLabel} className="hidden items-center gap-8 md:flex">
          {config.nav.items.map((item) => (
            <Link
              className="text-base font-semibold text-text transition hover:text-ink"
              href={localizePath(locale, item.href)}
              key={item.href}
            >
              {item.label}
            </Link>
          ))}
        </nav>
      </div>

      <div className="flex items-center gap-3">
        <span className="hidden rounded-full border border-border px-3 py-1.5 text-xs font-semibold text-muted lg:inline-flex">
          {config.nav.languageLabel}
        </span>
        <Link
          className="hidden h-11 items-center justify-center rounded-full border border-border bg-white px-5 text-sm font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10 md:inline-flex"
          href={localizePath(locale, config.nav.signIn.href)}
        >
          {config.nav.signIn.label}
        </Link>
        <Link
          className="inline-flex h-11 items-center justify-center rounded-full bg-ink px-4 text-sm font-semibold text-white transition hover:bg-text sm:px-5"
          href={localizePath(locale, config.nav.cta.href)}
        >
          {config.nav.cta.label}
        </Link>
        <MobileMenu
          ariaLabel={config.nav.ariaLabel}
          items={config.nav.items}
          locale={locale}
          menuCloseLabel={config.nav.menuCloseLabel}
          menuLabel={config.nav.menuLabel}
          signIn={config.nav.signIn}
        />
      </div>
      </div>
    </header>
  );
}
