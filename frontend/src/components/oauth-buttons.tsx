/**
 * "Continue with YouTube/Twitch" — plain links, not fetch. OAuth needs a
 * real browser navigation to the provider; the browser goes straight to the
 * backend (`API_URL`), same direct-to-backend rule as every other panel
 * request (see src/lib/api.ts).
 */
import { BrandMark } from "@/src/components/brand-mark";
import { API_URL } from "@/src/lib/api";

type Labels = {
  dividerLabel: string;
  google: string;
  twitch: string;
};

export function OAuthButtons({ labels }: { labels: Labels }) {
  return (
    <div className="mt-5">
      <div className="flex items-center gap-3 text-xs font-medium uppercase tracking-wide text-muted">
        <span className="h-px flex-1 bg-border" />
        {labels.dividerLabel}
        <span className="h-px flex-1 bg-border" />
      </div>
      <div className="mt-4 space-y-2">
        <a
          className="flex h-11 items-center justify-center gap-2 rounded-lg border border-border text-sm font-semibold text-ink transition hover:bg-canvas"
          href={`${API_URL}/api/auth/oauth/google/start`}
        >
          <BrandMark brand="youtube" className="h-5 w-5" />
          {labels.google}
        </a>
        <a
          className="flex h-11 items-center justify-center gap-2 rounded-lg border border-border text-sm font-semibold text-ink transition hover:bg-canvas"
          href={`${API_URL}/api/auth/oauth/twitch/start`}
        >
          <BrandMark brand="twitch" className="h-5 w-5" />
          {labels.twitch}
        </a>
      </div>
    </div>
  );
}
