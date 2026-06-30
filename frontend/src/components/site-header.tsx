import Image from "next/image";
import Link from "next/link";
import type { PublicConfig } from "@/src/lib/config";

type SiteHeaderProps = {
  config: PublicConfig;
};

export function SiteHeader({ config }: SiteHeaderProps) {
  return (
    <header className="mx-auto flex w-full max-w-7xl items-center justify-between px-4 py-5 sm:px-6 lg:px-8">
      <Link className="flex min-w-0 items-center gap-3" href="/">
        <Image
          alt={config.brand.name}
          className="h-11 w-auto object-contain"
          height={56}
          priority
          src={config.brand.logoAsset}
          width={230}
        />
      </Link>

      <nav aria-label="Primary navigation" className="hidden items-center gap-2 rounded-full border border-border bg-white/75 p-1 shadow-sm backdrop-blur md:flex">
        {config.nav.map((item) => (
          <Link
            className="rounded-full px-4 py-2 text-sm font-medium text-muted transition hover:bg-canvas hover:text-ink"
            href={item.href}
            key={item.href}
          >
            {item.label}
          </Link>
        ))}
      </nav>
    </header>
  );
}
