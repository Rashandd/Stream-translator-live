import Image from "next/image";
import Link from "next/link";
import type { PublicConfig } from "@/src/lib/config";
import { localizePath, type Locale } from "@/src/i18n/routing";

type SiteFooterProps = {
  config: PublicConfig;
  locale: Locale;
};

export function SiteFooter({ config, locale }: SiteFooterProps) {
  const year = new Date().getFullYear();

  return (
    <footer className="border-t border-hairline bg-white">
      <div className="mx-auto w-full max-w-7xl px-4 py-12 sm:px-6 lg:px-8 lg:py-16">
        <div className="grid gap-10 lg:grid-cols-[1.4fr_1fr_1fr]">
          <div className="max-w-sm">
            <Link className="flex items-center" href={localizePath(locale, "/")}>
              <Image
                alt={config.brand.name}
                className="h-10 w-auto object-contain"
                height={545}
                src={config.brand.logoAsset}
                width={2253}
              />
            </Link>
            <p className="mt-4 text-sm leading-6 text-muted">{config.footer.note}</p>
          </div>

          {config.footer.columns.map((column) => (
            <div key={column.title}>
              <h3 className="text-sm font-semibold text-ink">{column.title}</h3>
              <ul className="mt-4 flex flex-col gap-3">
                {column.items.map((item) => (
                  <li key={item.href}>
                    <Link
                      className="text-sm text-muted transition hover:text-ink"
                      href={localizePath(locale, item.href)}
                    >
                      {item.label}
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="mt-10 border-t border-hairline pt-6">
          <p className="text-sm text-faint">
            © {year} {config.footer.copyright}
          </p>
        </div>
      </div>
    </footer>
  );
}
