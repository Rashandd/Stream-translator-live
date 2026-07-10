"use client";

import { useCallback, useEffect, useState } from "react";
import { Icon } from "@/src/components/icon";
import { CopyRow } from "@/src/components/copy-row";
import { userFetch } from "@/src/lib/api";

/** `GET/PUT /api/relay/targets` kaydı (backend relay::RelayTarget). */
type RelayTarget = {
  platform: string;
  rtmp_url: string;
  stream_key: string;
};

// Faz 2: yayıncı hesabında henüz bağlı platform yok (YouTube/Twitch/Kick
// OAuth — "Platform Hesapları" ayrı, hâlâ kurulmamış bir iş). Bu kart o
// zamana kadar OBS'in SDK'nın yerel relay girişine bağlanması için TEK bir
// "obs" hedefi yönetir — sdk-tauri main.rs'in varsayılan dinleme adresiyle
// aynı port (STREAMDUBBER_RELAY_LISTEN_ADDR, bkz. sdk/src-tauri/src/main.rs).
const PRIMARY_PLATFORM = "obs";
const PRIMARY_RTMP_URL = "rtmp://127.0.0.1:19350/live";

function randomStreamKey(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  return `sd-live-${hex}`;
}

type Labels = {
  copyLabel: string;
  copiedLabel: string;
  regenerateLabel: string;
  loading: string;
  errors: {
    unauthorized: string;
    network: string;
    saveFailed: string;
  };
};

export function SdkStreamKeyPanel({ labels }: { labels: Labels }) {
  const [streamKey, setStreamKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const save = useCallback(async (key: string) => {
    return userFetch("/api/relay/targets", {
      method: "PUT",
      body: JSON.stringify({
        targets: [{ platform: PRIMARY_PLATFORM, rtmp_url: PRIMARY_RTMP_URL, stream_key: key }],
      }),
    });
  }, []);

  const load = useCallback(async () => {
    setError(null);
    try {
      const res = await userFetch("/api/relay/targets");
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      const targets = (await res.json()) as RelayTarget[];
      const existing = targets.find((target) => target.platform === PRIMARY_PLATFORM);
      if (existing) {
        setStreamKey(existing.stream_key);
        return;
      }
      // İlk ziyaret: gösterecek bir anahtar yok — kendiliğinden üret+kaydet.
      const key = randomStreamKey();
      const saveRes = await save(key);
      if (saveRes.ok) {
        setStreamKey(key);
      } else {
        setError(labels.errors.saveFailed);
      }
    } catch {
      setError(labels.errors.network);
    }
  }, [labels.errors, save]);

  useEffect(() => {
    void load();
  }, [load]);

  const regenerate = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const key = randomStreamKey();
      const res = await save(key);
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
      } else if (!res.ok) {
        setError(labels.errors.saveFailed);
      } else {
        setStreamKey(key);
      }
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusy(false);
    }
  }, [labels.errors, save]);

  return (
    <>
      <div className="mt-1 flex flex-col gap-2 sm:flex-row sm:items-center">
        <CopyRow
          className="flex-1"
          copiedLabel={labels.copiedLabel}
          copyLabel={labels.copyLabel}
          value={streamKey ?? labels.loading}
        />
        <button
          className="inline-flex h-10 shrink-0 items-center justify-center gap-1.5 rounded-full border border-border bg-surface px-4 text-sm font-semibold text-ink transition hover:border-danger hover:text-danger disabled:opacity-50"
          disabled={busy || !streamKey}
          onClick={() => void regenerate()}
          type="button"
        >
          <Icon className="h-4 w-4" name="KeyRound" />
          {labels.regenerateLabel}
        </button>
      </div>
      {error ? (
        <p className="mt-3 rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}
    </>
  );
}
