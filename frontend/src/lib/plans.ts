import { API_URL } from "@/src/lib/api";

/** Backend `plans` tablosuyla aynı alanlar (`backend/src/plans/mod.rs`). */
export type LivePlan = {
  name: string;
  price_cents: number;
  monthly_hours: number;
  max_languages: number;
  max_platforms: number;
  has_watermark: boolean;
  has_voice_clone: boolean;
};

/**
 * Canlı plan kataloğunu `GET /api/plans`'tan okur (public, auth gerekmez).
 * Backend erişilemezse `null` döner — çağıran taraf sözlükteki statik
 * fiyat/limitlere düşer, sayfa hiçbir zaman kırılmaz.
 */
export async function fetchLivePlans(): Promise<LivePlan[] | null> {
  try {
    const res = await fetch(`${API_URL}/api/plans`, { cache: "no-store" });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as LivePlan[];
  } catch {
    return null;
  }
}

export function formatPriceCents(cents: number): string {
  if (cents === 0) {
    return "$0";
  }
  return `$${(cents / 100).toFixed(2)}`;
}

export function formatHours(hours: number): string {
  return hours < 0 ? "Unlimited hours" : `${hours} hours a month`;
}

/**
 * Sözlükteki pazarlama metnini (isim, özellik cümleleri) korurken fiyat ve
 * "X hours a month" ifadesini canlı plan verisiyle değiştirir. Plan adı
 * eşleşmezse veya `live` `null`'sa o plan statik haliyle kalır.
 */
export function mergeLivePlans<T extends { name: string; price: string; features: string[] }>(
  dictionaryPlans: T[],
  live: LivePlan[] | null,
): T[] {
  if (!live) {
    return dictionaryPlans;
  }
  const byName = new Map(live.map((plan) => [plan.name.toLowerCase(), plan]));
  return dictionaryPlans.map((plan) => {
    const match = byName.get(plan.name.toLowerCase());
    if (!match) {
      return plan;
    }
    return {
      ...plan,
      price: formatPriceCents(match.price_cents),
      features: plan.features.map((feature) =>
        /^\d+ hours a month/.test(feature)
          ? feature.replace(/^\d+ hours a month/, formatHours(match.monthly_hours))
          : feature,
      ),
    };
  });
}
