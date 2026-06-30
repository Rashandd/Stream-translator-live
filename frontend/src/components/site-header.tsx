import Image from "next/image";
import Link from "next/link";
import type { PublicConfig } from "@/src/lib/config";
import { localizePath, type Locale } from "@/src/i18n/routing";

type SiteHeaderProps = {
  config: PublicConfig;
  locale: Locale;
};

export function SiteHeader({ config, locale }: SiteHeaderProps) {
  return (
    <header className="border-b border-hairline bg-white/86 backdrop-blur">
      <div className="mx-auto flex w-full max-w-7xl items-center justify-between px-4 py-4 sm:px-6 lg:px-8">
      <Link className="flex min-w-0 items-center gap-3" href={localizePath(locale, "/")}>
        <Image
          alt={config.brand.name}
          className="h-16 w-auto object-contain sm:h-20"
          height={92}
          priority
          src={config.brand.logoAsset}
          width={330}
        />
      </Link>

      <nav aria-label={config.nav.ariaLabel} className="hidden items-center gap-8 md:flex">
        {config.nav.items.map((item) => (
          <Link
            className="text-sm font-semibold text-muted transition hover:text-ink"
            href={localizePath(locale, item.href)}
            key={item.href}
          >
            {item.label}
          </Link>
        ))}
      </nav>

      <div className="flex items-center gap-3">
        <span className="hidden rounded-full border border-border px-3 py-1.5 text-xs font-semibold text-muted sm:inline-flex">
          {config.nav.languageLabel}
        </span>
        <Link
          className="inline-flex h-11 items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text"
          href={localizePath(locale, config.nav.cta.href)}
        >
          {config.nav.cta.label}
        </Link>
      </div>
      </div>
    </header>
  );
}
