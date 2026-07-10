import { userFetch } from "@/src/lib/api";

export type FoundingPlatform = "kick" | "youtube" | "twitch" | "other";
export type FoundingApplicationStatus = "pending" | "approved" | "revoked";

export type FoundingApplication = {
  platform: FoundingPlatform;
  channel_url: string;
  message: string;
  status: FoundingApplicationStatus;
  auto_created_via_oauth: boolean;
};

export type FoundingGateStatus = {
  /** `founding.gate.enabled` — kapalıyken herkes akışa erişebilir. */
  enabled: boolean;
  /** Yayıncının kendi başvurusu (yoksa `null`). */
  application: FoundingApplication | null;
};

/** `GET /api/founding/gate` + `GET /api/founding/application`'ı paralel çeker. */
export async function fetchFoundingStatus(): Promise<FoundingGateStatus> {
  const [gateRes, appRes] = await Promise.all([
    userFetch("/api/founding/gate"),
    userFetch("/api/founding/application"),
  ]);
  const enabled = gateRes.ok ? ((await gateRes.json()) as { enabled: boolean }).enabled : false;
  const application = appRes.ok ? ((await appRes.json()) as FoundingApplication) : null;
  return { enabled, application };
}
