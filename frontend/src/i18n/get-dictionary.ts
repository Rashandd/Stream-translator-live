import en from "@/src/i18n/dictionaries/en.json";
import { defaultLocale, isLocale, type Locale } from "@/src/i18n/routing";

const dictionaries = {
  en,
} satisfies Record<Locale, typeof en>;

export type Dictionary = typeof en;

export async function getDictionary(locale: string): Promise<Dictionary> {
  const safeLocale = isLocale(locale) ? locale : defaultLocale;

  return dictionaries[safeLocale];
}
