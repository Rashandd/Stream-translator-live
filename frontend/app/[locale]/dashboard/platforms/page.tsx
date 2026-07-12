import { notFound, redirect } from "next/navigation";
import { isLocale, locales, localizePath } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function PlatformsRedirectPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  redirect(localizePath(rawLocale, "/dashboard/account"));
}
