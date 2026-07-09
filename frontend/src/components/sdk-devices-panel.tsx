"use client";

import { useCallback, useEffect, useState } from "react";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";

/** `GET /api/sdk/devices` yanıtı (backend sdk_session::DeviceDto). */
type Device = {
  id: string;
  device_name: string;
  sdk_version: string;
  paired_at: number;
  last_seen_at: number;
  connected: boolean;
};

type Labels = {
  emptyState: string;
  connectedLabel: string;
  offlineLabel: string;
  pairedTemplate: string;
  lastSeenTemplate: string;
  removeLabel: string;
  removingLabel: string;
  errors: {
    unauthorized: string;
    network: string;
    failed: string;
  };
};

/** Panel canlı durumu tazelemek için bu aralıkla yeniden çeker (SDK ~20sn'de
 * bir heartbeat atıyor, bkz. `sdk/src/core/heartbeat.rs`). */
const POLL_INTERVAL_MS = 15_000;

function fill(template: string, values: Record<string, string>): string {
  return Object.entries(values).reduce(
    (acc, [key, value]) => acc.replaceAll(`{${key}}`, value),
    template,
  );
}

function relativeTime(unixSeconds: number): string {
  const deltaSecs = Math.max(0, Math.round(Date.now() / 1000 - unixSeconds));
  if (deltaSecs < 60) return `${deltaSecs}s`;
  const minutes = Math.round(deltaSecs / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.round(hours / 24)}d`;
}

export function SdkDevicesPanel({ labels }: { labels: Labels }) {
  const [devices, setDevices] = useState<Device[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [removingId, setRemovingId] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const res = await userFetch("/api/sdk/devices");
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      setError(null);
      setDevices((await res.json()) as Device[]);
    } catch {
      setError(labels.errors.network);
    }
  }, [labels.errors]);

  useEffect(() => {
    void load();
    const interval = setInterval(() => void load(), POLL_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [load]);

  const remove = useCallback(
    async (id: string) => {
      setRemovingId(id);
      setError(null);
      try {
        const res = await userFetch(`/api/sdk/devices/${id}`, { method: "DELETE" });
        if (res.status === 401) {
          setError(labels.errors.unauthorized);
          return;
        }
        if (!res.ok && res.status !== 404) {
          setError(labels.errors.failed);
          return;
        }
        setDevices((prev) => (prev ? prev.filter((d) => d.id !== id) : prev));
      } catch {
        setError(labels.errors.network);
      } finally {
        setRemovingId(null);
      }
    },
    [labels.errors],
  );

  if (devices === null && !error) {
    return null;
  }

  return (
    <>
      {error ? (
        <p className="mb-3 rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}
      {devices && devices.length === 0 ? (
        <p className="rounded-lg border border-hairline bg-canvas px-4 py-3 text-sm text-muted">{labels.emptyState}</p>
      ) : null}
      {devices && devices.length > 0 ? (
        <ul className="space-y-2">
          {devices.map((device) => (
            <li
              className="flex items-center justify-between gap-3 rounded-lg border border-hairline bg-canvas px-4 py-3"
              key={device.id}
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span
                    aria-hidden
                    className={`h-1.5 w-1.5 shrink-0 rounded-full ${device.connected ? "bg-success" : "bg-muted"}`}
                  />
                  <span className="truncate text-sm font-semibold text-ink">{device.device_name}</span>
                  <span className="text-xs font-medium text-muted">
                    {device.connected ? labels.connectedLabel : labels.offlineLabel}
                  </span>
                </div>
                <p className="mt-0.5 text-xs text-muted">
                  {fill(labels.pairedTemplate, { time: relativeTime(device.paired_at) })}
                  {" · "}
                  {fill(labels.lastSeenTemplate, { time: relativeTime(device.last_seen_at) })}
                </p>
              </div>
              <button
                className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-full border border-border bg-white px-3 text-xs font-semibold text-muted transition hover:border-danger hover:text-danger disabled:opacity-50"
                disabled={removingId === device.id}
                onClick={() => void remove(device.id)}
                type="button"
              >
                <Icon className="h-3.5 w-3.5" name="X" />
                {removingId === device.id ? labels.removingLabel : labels.removeLabel}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </>
  );
}
