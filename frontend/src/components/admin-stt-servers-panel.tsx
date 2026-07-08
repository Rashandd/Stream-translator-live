"use client";

import { useCallback, useEffect, useState } from "react";
import { adminFetch } from "@/src/lib/api";
import { CopyRow } from "@/src/components/copy-row";
import { Icon } from "@/src/components/icon";

type SttServerStatus = "pending" | "provisioning" | "ready" | "error";

type SttServer = {
  id: string;
  name: string;
  hostname: string;
  ssh_port: number;
  ssh_user: string;
  ssh_public_key: string;
  host_fingerprint: string | null;
  public_endpoint_override: string | null;
  status: SttServerStatus;
  status_detail: string | null;
  endpoint_url: string | null;
  gpu_name: string | null;
  vram_mb: number | null;
  last_health_check_at: number | null;
  created_at: number;
};

type Labels = {
  addTitle: string;
  nameLabel: string;
  namePlaceholder: string;
  hostnameLabel: string;
  hostnamePlaceholder: string;
  sshPortLabel: string;
  sshUserLabel: string;
  publicEndpointLabel: string;
  publicEndpointPlaceholder: string;
  publicEndpointHint: string;
  addButton: string;
  addingButton: string;
  publicKeyIntro: string;
  copyLabel: string;
  copiedLabel: string;
  columns: { name: string; status: string; gpu: string; actions: string };
  statusLabels: Record<SttServerStatus, string>;
  provisionButton: string;
  provisioningButton: string;
  deleteButton: string;
  confirmDelete: string;
  loading: string;
  empty: string;
  errors: { unauthorized: string; network: string; addFailed: string; provisionFailed: string };
  signInLabel: string;
};

type Props = {
  labels: Labels;
  /** 401 durumunda gösterilecek locale'li admin login yolu. */
  loginHref: string;
};

const statusTone: Record<SttServerStatus, string> = {
  pending: "border-border bg-canvas",
  provisioning: "border-blue/25 bg-blue/10",
  ready: "border-success/25 bg-success/10",
  error: "border-danger/25 bg-danger/10",
};

const POLL_INTERVAL_MS = 3000;

export function AdminSttServersPanel({ labels, loginHref }: Props) {
  const [servers, setServers] = useState<SttServer[] | null>(null);
  const [unauthorized, setUnauthorized] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [name, setName] = useState("");
  const [hostname, setHostname] = useState("");
  const [sshPort, setSshPort] = useState("22");
  const [sshUser, setSshUser] = useState("root");
  const [publicEndpointOverride, setPublicEndpointOverride] = useState("");
  const [adding, setAdding] = useState(false);
  const [justAdded, setJustAdded] = useState<SttServer | null>(null);

  const [busyId, setBusyId] = useState<string | null>(null);

  const applyListResponse = useCallback(async (response: Response) => {
    if (response.status === 401 || response.status === 403) {
      setUnauthorized(true);
      setServers([]);
      return;
    }
    if (!response.ok) {
      return;
    }
    setUnauthorized(false);
    setServers((await response.json()) as SttServer[]);
  }, []);

  const load = useCallback(async () => {
    try {
      await applyListResponse(await adminFetch("/admin/stt-servers"));
    } catch {
      setError(labels.errors.network);
      setServers([]);
    }
  }, [applyListResponse, labels.errors.network]);

  useEffect(() => {
    void load();
  }, [load]);

  // Sunuculardan biri "provisioning" durumundaysa kısa aralıkla poll et —
  // kurulum dakikalar sürebilir (model indirme + CUDA warm-up dahil).
  useEffect(() => {
    const hasProvisioning = servers?.some((s) => s.status === "provisioning") ?? false;
    if (!hasProvisioning) {
      return;
    }
    const timer = window.setInterval(() => void load(), POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [servers, load]);

  async function addServer() {
    if (!name.trim() || !hostname.trim()) {
      return;
    }
    setAdding(true);
    setError(null);
    try {
      const response = await adminFetch("/admin/stt-servers", {
        method: "POST",
        body: JSON.stringify({
          name: name.trim(),
          hostname: hostname.trim(),
          ssh_port: Number(sshPort) || 22,
          ssh_user: sshUser.trim() || "root",
          public_endpoint_override: publicEndpointOverride.trim() || null,
        }),
      });
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!response.ok) {
        setError(labels.errors.addFailed);
        return;
      }
      const created = (await response.json()) as SttServer;
      setJustAdded(created);
      setName("");
      setHostname("");
      setSshPort("22");
      setSshUser("root");
      setPublicEndpointOverride("");
      await load();
    } catch {
      setError(labels.errors.network);
    } finally {
      setAdding(false);
    }
  }

  async function provision(id: string) {
    setBusyId(id);
    setError(null);
    try {
      const response = await adminFetch(`/admin/stt-servers/${id}/provision`, { method: "POST" });
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (!response.ok && response.status !== 202) {
        setError(labels.errors.provisionFailed);
        return;
      }
      await load();
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusyId(null);
    }
  }

  async function remove(id: string) {
    if (!window.confirm(labels.confirmDelete)) {
      return;
    }
    setBusyId(id);
    try {
      const response = await adminFetch(`/admin/stt-servers/${id}`, { method: "DELETE" });
      if (response.status === 401 || response.status === 403) {
        setUnauthorized(true);
        return;
      }
      if (response.ok || response.status === 204) {
        if (justAdded?.id === id) {
          setJustAdded(null);
        }
        await load();
      }
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusyId(null);
    }
  }

  if (unauthorized) {
    return (
      <div className="rounded-xl border border-warning/25 bg-warning/10 p-4">
        <p className="text-sm text-ink">{labels.errors.unauthorized}</p>
        <a
          className="mt-3 inline-flex h-9 items-center rounded-full bg-ink px-4 text-xs font-semibold text-white transition hover:bg-text"
          href={loginHref}
        >
          {labels.signInLabel}
        </a>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {error ? (
        <p className="rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-ink" role="alert">
          {error}
        </p>
      ) : null}

      <div className="rounded-xl border border-border bg-white p-4">
        <h2 className="text-sm font-semibold text-ink">{labels.addTitle}</h2>
        <div className="mt-3 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <label className="flex flex-col gap-1 text-xs font-semibold text-muted">
            {labels.nameLabel}
            <input
              className="h-9 rounded-lg border border-border bg-white px-2.5 text-sm text-ink outline-none transition focus:border-blue"
              onChange={(event) => setName(event.target.value)}
              placeholder={labels.namePlaceholder}
              value={name}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-muted">
            {labels.hostnameLabel}
            <input
              className="h-9 rounded-lg border border-border bg-white px-2.5 font-mono text-sm text-ink outline-none transition focus:border-blue"
              onChange={(event) => setHostname(event.target.value)}
              placeholder={labels.hostnamePlaceholder}
              value={hostname}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-muted">
            {labels.sshPortLabel}
            <input
              className="h-9 rounded-lg border border-border bg-white px-2.5 font-mono text-sm text-ink outline-none transition focus:border-blue"
              onChange={(event) => setSshPort(event.target.value)}
              type="number"
              value={sshPort}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-muted">
            {labels.sshUserLabel}
            <input
              className="h-9 rounded-lg border border-border bg-white px-2.5 font-mono text-sm text-ink outline-none transition focus:border-blue"
              onChange={(event) => setSshUser(event.target.value)}
              value={sshUser}
            />
          </label>
        </div>
        <label className="mt-3 flex flex-col gap-1 text-xs font-semibold text-muted">
          {labels.publicEndpointLabel}
          <input
            className="h-9 w-full rounded-lg border border-border bg-white px-2.5 font-mono text-sm text-ink outline-none transition focus:border-blue"
            onChange={(event) => setPublicEndpointOverride(event.target.value)}
            placeholder={labels.publicEndpointPlaceholder}
            value={publicEndpointOverride}
          />
        </label>
        <p className="mt-1 text-xs text-faint">{labels.publicEndpointHint}</p>
        <button
          className="mt-3 inline-flex h-9 items-center rounded-full bg-ink px-4 text-xs font-semibold text-white transition hover:bg-text disabled:opacity-50"
          disabled={adding || !name.trim() || !hostname.trim()}
          onClick={() => void addServer()}
          type="button"
        >
          {adding ? labels.addingButton : labels.addButton}
        </button>

        {justAdded ? (
          <div className="mt-4 rounded-lg border border-blue/25 bg-blue/10 p-3">
            <p className="text-sm text-ink">{labels.publicKeyIntro}</p>
            <div className="mt-2">
              <CopyRow
                copiedLabel={labels.copiedLabel}
                copyLabel={labels.copyLabel}
                value={justAdded.ssh_public_key}
              />
            </div>
          </div>
        ) : null}
      </div>

      <div className="overflow-hidden rounded-xl border border-border bg-white">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[760px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                <th className="px-4 py-3 font-semibold">{labels.columns.name}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.status}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.gpu}</th>
                <th className="px-4 py-3 font-semibold">{labels.columns.actions}</th>
              </tr>
            </thead>
            <tbody>
              {servers === null ? (
                <tr>
                  <td className="px-4 py-6 text-sm text-muted" colSpan={4}>
                    {labels.loading}
                  </td>
                </tr>
              ) : null}
              {servers?.map((server) => (
                <tr className="border-b border-hairline last:border-0" key={server.id}>
                  <td className="px-4 py-3">
                    <div className="font-semibold text-ink">{server.name}</div>
                    <div className="font-mono text-xs text-muted">
                      {server.ssh_user}@{server.hostname}:{server.ssh_port}
                    </div>
                    {server.public_endpoint_override ? (
                      <div className="font-mono text-xs text-faint">{server.public_endpoint_override}</div>
                    ) : null}
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold text-ink ${statusTone[server.status]}`}
                      title={server.status_detail ?? undefined}
                    >
                      {labels.statusLabels[server.status]}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-xs text-muted">
                    {server.gpu_name ? (
                      <>
                        {server.gpu_name}
                        {server.vram_mb ? ` · ${(server.vram_mb / 1024).toFixed(1)} GB` : ""}
                      </>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap items-center gap-2">
                      {server.status !== "ready" ? (
                        <button
                          className="inline-flex h-9 items-center gap-1.5 rounded-full border border-blue/40 px-3 text-xs font-semibold text-ink transition hover:bg-blue/10 disabled:opacity-50"
                          disabled={busyId === server.id || server.status === "provisioning"}
                          onClick={() => void provision(server.id)}
                          type="button"
                        >
                          <Icon className="h-3.5 w-3.5" name="RefreshCw" />
                          {server.status === "provisioning" ? labels.provisioningButton : labels.provisionButton}
                        </button>
                      ) : null}
                      <button
                        className="inline-flex h-9 items-center gap-1.5 rounded-full border border-border px-3 text-xs font-semibold text-muted transition hover:text-ink disabled:opacity-50"
                        disabled={busyId === server.id}
                        onClick={() => void remove(server.id)}
                        type="button"
                      >
                        <Icon className="h-3.5 w-3.5" name="X" />
                        {labels.deleteButton}
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
              {servers && servers.length === 0 ? (
                <tr>
                  <td className="px-4 py-6 text-sm text-muted" colSpan={4}>
                    {labels.empty}
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
