"use client";

import { useCallback, useEffect, useState } from "react";
import { adminFetch } from "@/src/lib/api";

/** `GET /admin/voice/models` kaydı (backend voice_models::VoiceModel). */
type VoiceModel = {
  id: string;
  user_id: string;
  name: string;
  status: "processing" | "pending_review" | "approved" | "rejected";
  created_at: number;
};

type Labels = {
  loading: string;
  empty: string;
  columns: { user: string; name: string; status: string; created: string; actions: string };
  statusLabels: Record<VoiceModel["status"], string>;
  approveLabel: string;
  rejectLabel: string;
  errors: { unauthorized: string; network: string; actionFailed: string };
};

const statusTone: Record<VoiceModel["status"], string> = {
  processing: "border-border bg-canvas text-muted",
  pending_review: "border-warning/25 bg-warning/10 text-ink",
  approved: "border-success/25 bg-success/10 text-ink",
  rejected: "border-danger/25 bg-danger/10 text-ink",
};

export function AdminVoiceModelsPanel({ labels }: { labels: Labels }) {
  const [models, setModels] = useState<VoiceModel[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const res = await adminFetch("/admin/voice/models");
      if (res.status === 401 || res.status === 403) {
        setError(labels.errors.unauthorized);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      setModels((await res.json()) as VoiceModel[]);
    } catch {
      setError(labels.errors.network);
    }
  }, [labels.errors]);

  useEffect(() => {
    void load();
  }, [load]);

  const act = useCallback(
    async (id: string, action: "approve" | "reject") => {
      setBusyId(id);
      setError(null);
      try {
        const res = await adminFetch(`/admin/voice/models/${id}/${action}`, { method: "POST" });
        if (!res.ok) {
          setError(labels.errors.actionFailed);
          return;
        }
        await load();
      } catch {
        setError(labels.errors.network);
      } finally {
        setBusyId(null);
      }
    },
    [labels.errors.actionFailed, labels.errors.network, load],
  );

  if (!models && !error) {
    return <p className="text-sm text-muted">{labels.loading}</p>;
  }

  return (
    <div className="space-y-3">
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-4 py-3 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      {!models || models.length === 0 ? (
        <p className="text-sm text-muted">{labels.empty}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[640px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs font-semibold uppercase tracking-wide text-muted">
                <th className="pb-2">{labels.columns.user}</th>
                <th className="pb-2">{labels.columns.name}</th>
                <th className="pb-2">{labels.columns.status}</th>
                <th className="pb-2">{labels.columns.created}</th>
                <th className="pb-2 text-right">{labels.columns.actions}</th>
              </tr>
            </thead>
            <tbody>
              {models.map((model) => (
                <tr className="border-b border-hairline last:border-0" key={model.id}>
                  <td className="py-2 font-mono text-xs text-muted">{model.user_id}</td>
                  <td className="py-2 font-semibold text-ink">{model.name}</td>
                  <td className="py-2">
                    <span
                      className={`inline-flex rounded-full border px-2.5 py-1 text-xs font-semibold ${statusTone[model.status]}`}
                    >
                      {labels.statusLabels[model.status]}
                    </span>
                  </td>
                  <td className="py-2 text-xs text-muted">
                    {new Date(model.created_at * 1000).toLocaleString()}
                  </td>
                  <td className="py-2 text-right">
                    <div className="inline-flex gap-2">
                      <button
                        className="inline-flex h-8 items-center rounded-full bg-ink px-3 text-xs font-semibold text-white transition hover:bg-text disabled:opacity-50"
                        disabled={busyId === model.id || model.status === "approved"}
                        onClick={() => void act(model.id, "approve")}
                        type="button"
                      >
                        {labels.approveLabel}
                      </button>
                      <button
                        className="inline-flex h-8 items-center rounded-full border border-border px-3 text-xs font-semibold text-ink transition hover:border-danger hover:text-danger disabled:opacity-50"
                        disabled={busyId === model.id || model.status === "rejected"}
                        onClick={() => void act(model.id, "reject")}
                        type="button"
                      >
                        {labels.rejectLabel}
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
