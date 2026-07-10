import type { Metadata } from "next";
import { headers } from "next/headers";
import type { ReactNode } from "react";
import en from "@/src/i18n/dictionaries/en.json";
import "./globals.css";

export const metadata: Metadata = {
  title: { template: `%s · ${en.brand.name}`, default: en.brand.name },
  description: en.brand.tagline,
};

export default async function RootLayout({
  children,
}: Readonly<{
  children: ReactNode;
}>) {
  const nonce = (await headers()).get("x-nonce") ?? undefined;

  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        {/* Browsers strip the real nonce from the DOM attribute after parsing
            (security feature), so React's hydration diff always sees a false
            mismatch here — suppress it rather than the underlying warning. */}
        {/* eslint-disable-next-line @next/next/no-sync-scripts */}
        <script nonce={nonce} src="/theme-init.js" suppressHydrationWarning />
      </head>
      <body suppressHydrationWarning>{children}</body>
    </html>
  );
}
