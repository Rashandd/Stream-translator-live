import { API_URL } from "@/src/lib/api";

export type FoundingPromo = {
  enabled: boolean;
  message: string;
};

/** `GET /config`'in düz key/value snapshot'ından promo alanlarını çıkarır. */
export function parseFoundingPromo(config: Record<string, string>): FoundingPromo {
  return {
    enabled: config["promo.founding.enabled"] === "true",
    message: config["promo.founding.message"] ?? "",
  };
}

/**
 * Founding beta promosyon durumunu `GET /config`'ten okur (public, auth
 * gerekmez — backend `config.rs::with_defaults()` ile `promo.founding.*`
 * anahtarlarını seed'ler, admin panelden `PUT /admin/config` ile
 * restart'sız değişir). Backend erişilemezse `null` döner — çağıran taraf
 * banner'ı/üstü çizili fiyatı hiç göstermez, sayfa hiçbir zaman kırılmaz.
 */
export async function fetchFoundingPromo(): Promise<FoundingPromo | null> {
  try {
    const res = await fetch(`${API_URL}/config`, { cache: "no-store" });
    if (!res.ok) {
      return null;
    }
    const config = (await res.json()) as Record<string, string>;
    return parseFoundingPromo(config);
  } catch {
    return null;
  }
}
