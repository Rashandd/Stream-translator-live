import type { Metadata } from "next";
import type { ReactNode } from "react";
import en from "@/src/i18n/dictionaries/en.json";
import { THEME_INIT_SCRIPT } from "@/src/lib/theme";
import "./globals.css";

export const metadata: Metadata = {
  title: { template: `%s · ${en.brand.name}`, default: en.brand.name },
  description: en.brand.tagline,
};

export default function RootLayout({
  children,
}: Readonly<{
  children: ReactNode;
}>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
      </head>
      <body suppressHydrationWarning>{children}</body>
    </html>
  );
}
