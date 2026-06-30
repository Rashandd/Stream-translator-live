import { getDictionary } from "@/src/i18n/get-dictionary";
import { defaultLocale, type Locale } from "@/src/i18n/routing";

export type PublicConfig = Awaited<ReturnType<typeof getDictionary>>;

export async function getPublicConfig(locale: Locale = defaultLocale): Promise<PublicConfig> {
  return getDictionary(locale);
}
