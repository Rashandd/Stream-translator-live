"use client";

import { useEffect, useState, type FormEvent } from "react";
import { SectionCard } from "@/src/components/dashboard-ui";
import { userFetch } from "@/src/lib/api";

type Platform = "kick" | "youtube" | "twitch" | "other";
type Status = "pending" | "approved" | "revoked";

type Application = {
  platform: Platform;
  channel_url: string;
  message: string;
  status: Status;
  auto_created_via_oauth: boolean;
};

type Labels = {
  title: string;
  subtitle: string;
  platformLabel: string;
  platformOptions: Record<Platform, string>;
  channelUrlLabel: string;
  channelUrlPlaceholder: string;
  messageLabel: string;
  messagePlaceholder: string;
  submitLabel: string;
  submittingLabel: string;
  editLabel: string;
  statusLabels: Record<Status, string>;
  statusHints: Record<Status, string>;
  connectCta: string;
  connectYoutubeLabel: string;
  connectTwitchLabel: string;
  manualFallbackLabel: string;
  autoVerifiedBadge: string;
  errors: {
    network: string;
    invalid: string;
  };
};

type LoadState = "loading" | "none" | "loaded";

export function FoundingApplicationPanel({ labels }: { labels: Labels }) {
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [application, setApplication] = useState<Application | null>(null);
  const [editing, setEditing] = useState(false);
  const [showManualForm, setShowManualForm] = useState(false);
  const [platform, setPlatform] = useState<Platform>("kick");
  const [channelUrl, setChannelUrl] = useState("");
  const [message, setMessage] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connectBusy, setConnectBusy] = useState<"youtube" | "twitch" | null>(null);

  useEffect(() => {
    let cancelled = false;
    userFetch("/api/founding/application")
      .then(async (res) => {
        if (cancelled) return;
        if (res.status === 404) {
          setLoadState("none");
          return;
        }
        if (!res.ok) {
          setLoadState("none");
          return;
        }
        const data = (await res.json()) as Application;
        setApplication(data);
        setPlatform(data.platform);
        setChannelUrl(data.channel_url);
        setMessage(data.message);
        setLoadState("loaded");
      })
      .catch(() => {
        if (!cancelled) setLoadState("none");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function connect(provider: "youtube" | "twitch") {
    setConnectBusy(provider);
    setError(null);
    try {
      const res = await userFetch(`/api/integrations/${provider}/authorize`);
      if (!res.ok) {
        setError(labels.errors.network);
        return;
      }
      const data = (await res.json()) as { authorize_url: string };
      window.location.href = data.authorize_url;
    } catch {
      setError(labels.errors.network);
    } finally {
      setConnectBusy(null);
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      const res = await userFetch("/api/founding/application", {
        method: "POST",
        body: JSON.stringify({ platform, channel_url: channelUrl, message }),
      });
      if (res.status === 422) {
        setError(labels.errors.invalid);
        setSubmitting(false);
        return;
      }
      if (!res.ok) {
        setError(labels.errors.network);
        setSubmitting(false);
        return;
      }
      const data = (await res.json()) as Application;
      setApplication(data);
      setLoadState("loaded");
      setEditing(false);
      setSubmitting(false);
    } catch {
      setError(labels.errors.network);
      setSubmitting(false);
    }
  }

  if (loadState === "loading") {
    return null;
  }

  if (loadState === "loaded" && application && !editing) {
    return (
      <SectionCard subtitle={labels.subtitle} title={labels.title}>
        <div className="flex flex-wrap items-center gap-3">
          <span
            className={`rounded-full px-2.5 py-0.5 text-xs font-semibold ${
              application.status === "approved"
                ? "bg-success/10 text-success"
                : application.status === "revoked"
                  ? "bg-danger/10 text-danger"
                  : "bg-warning/10 text-warning"
            }`}
          >
            {labels.statusLabels[application.status]}
          </span>
          {application.auto_created_via_oauth ? (
            <span className="rounded-full bg-blue/10 px-2.5 py-0.5 text-xs font-semibold text-blue">
              {labels.autoVerifiedBadge}
            </span>
          ) : null}
          <p className="text-sm text-muted">{labels.platformOptions[application.platform]}</p>
          <a
            className="text-sm text-blue underline"
            href={application.channel_url}
            rel="noreferrer"
            target="_blank"
          >
            {application.channel_url}
          </a>
        </div>
        <p className="mt-3 text-sm leading-6 text-muted">{labels.statusHints[application.status]}</p>
        <button
          className="mt-4 inline-flex h-9 items-center justify-center rounded-full border border-border bg-surface px-4 text-xs font-semibold text-ink transition hover:border-cyan hover:bg-cyan/10"
          onClick={() => setEditing(true)}
          type="button"
        >
          {labels.editLabel}
        </button>
      </SectionCard>
    );
  }

  if (loadState === "none" && !showManualForm) {
    return (
      <SectionCard subtitle={labels.subtitle} title={labels.title}>
        <p className="text-sm leading-6 text-muted">{labels.connectCta}</p>
        <div className="mt-4 flex flex-wrap gap-2">
          <button
            className="inline-flex h-10 items-center justify-center gap-1.5 rounded-full bg-primary px-4 text-sm font-semibold text-white transition hover:bg-primary-hover disabled:opacity-60"
            disabled={connectBusy === "youtube"}
            onClick={() => void connect("youtube")}
            type="button"
          >
            {labels.connectYoutubeLabel}
          </button>
          <button
            className="inline-flex h-10 items-center justify-center gap-1.5 rounded-full bg-primary px-4 text-sm font-semibold text-white transition hover:bg-primary-hover disabled:opacity-60"
            disabled={connectBusy === "twitch"}
            onClick={() => void connect("twitch")}
            type="button"
          >
            {labels.connectTwitchLabel}
          </button>
        </div>
        {error ? <p className="mt-3 text-sm text-danger">{error}</p> : null}
        <button
          className="mt-4 text-xs font-medium text-muted underline"
          onClick={() => setShowManualForm(true)}
          type="button"
        >
          {labels.manualFallbackLabel}
        </button>
      </SectionCard>
    );
  }

  return (
    <SectionCard subtitle={labels.subtitle} title={labels.title}>
      <form className="space-y-4" onSubmit={submit}>
        <div>
          <label className="block text-xs font-medium text-muted" htmlFor="founding-platform">
            {labels.platformLabel}
          </label>
          <select
            className="mt-1 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink"
            id="founding-platform"
            onChange={(e) => setPlatform(e.target.value as Platform)}
            value={platform}
          >
            {(Object.keys(labels.platformOptions) as Platform[]).map((key) => (
              <option key={key} value={key}>
                {labels.platformOptions[key]}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-xs font-medium text-muted" htmlFor="founding-channel-url">
            {labels.channelUrlLabel}
          </label>
          <input
            className="mt-1 h-10 w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink"
            id="founding-channel-url"
            onChange={(e) => setChannelUrl(e.target.value)}
            placeholder={labels.channelUrlPlaceholder}
            type="text"
            value={channelUrl}
          />
        </div>
        <div>
          <label className="block text-xs font-medium text-muted" htmlFor="founding-message">
            {labels.messageLabel}
          </label>
          <textarea
            className="mt-1 w-full rounded-lg border border-border bg-surface px-3 py-2 text-sm text-ink"
            id="founding-message"
            onChange={(e) => setMessage(e.target.value)}
            placeholder={labels.messagePlaceholder}
            rows={3}
            value={message}
          />
        </div>
        {error ? <p className="text-sm text-danger">{error}</p> : null}
        <button
          className="inline-flex h-10 items-center justify-center rounded-full bg-primary px-5 text-sm font-semibold text-white transition hover:bg-primary-hover disabled:opacity-60"
          disabled={submitting || channelUrl.trim().length === 0}
          type="submit"
        >
          {submitting ? labels.submittingLabel : labels.submitLabel}
        </button>
      </form>
    </SectionCard>
  );
}
