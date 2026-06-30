import type { Metadata } from "next";
import type { ReactNode } from "react";
import publicConfig from "@/src/config/public-config.json";
import "./globals.css";

export const metadata: Metadata = {
  title: publicConfig.brand.name,
  description: publicConfig.brand.tagline,
};

export default function RootLayout({
  children,
}: Readonly<{
  children: ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
