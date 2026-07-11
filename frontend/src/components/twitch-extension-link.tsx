"use client";

import { useCallback, useEffect, useState } from "react";
import { adminFetch } from "@/src/lib/api";
import { Icon } from "@/src/components/icon";

/**
 * Twitch Extension kanal bağlama kartının interaktif kısmı.
 *
 * Backend uçları gerçek ve testli (`/api/ext/twitch/code|binding`). Yayıncı
 * hesap sistemi (1.4 user backend'i) henüz olmadığı için oturum kaynağı,
 * V11 Google Connect emsalindeki gibi admin bootstrap oturumudur
 * (`adminFetch`): operatör akışı uçtan uca bugün doğrular, broadcaster
 * girişi gelince yalnız oturum kaynağı değişir. Bu kart yalnızca
 * `AuthGuard` ile korunan admin sayfalarında render edilir; oturum HttpOnly
 * çerezde olduğu için client-side bir ön-kontrol mümkün değil — ilk fetch
 * denenir, 401/403 gelirse `hasSession` false'a düşer ve `signInNote`
 * gösterilir.
 */

type Labels = {
  codeButton: string;
  generatingLabel: string;
  codeReadyLabel: string;
  validForLabel: string;
  copyLabel: string;
  copiedLabel: string;
  linkedLabel: string;
  notLinkedLabel: string;
  unlinkLabel: string;
  confirmUnlink: string;
  signInNote: string;
  disabledNote: string;
  errors: { network: string; failed: string };
};

type Props = { labels: Labels };

type Binding = { linked: boolean; channels: string[] };

type LinkCode = { code: string; expiresAt: number };

export function TwitchExtensionLink({ labels }: Props) {
  const [hasSession, setHasSession] = useState(true);
  const [serviceOff, setServiceOff] = useState(false);
  const [binding, setBinding] = useState<Binding | null>(null);
  const [code, setCode] = useState<LinkCode | null>(null);
  const [remaining, setRemaining] = useState(0);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** 401/403 → oturum yok; 503 → servis kapalı. Ortak yanıt eleme. */
  const gate = useCallback((status: number): boolean => {
    if (status === 401 || status === 403) {
      setHasSession(false);
      return true;
    }
    if (status === 503) {
      setServiceOff(true);
      return true;
    }
    return false;
  }, []);

  const refreshBinding = useCallback(async () => {
    try {
      const resp = await adminFetch("/api/ext/twitch/binding");
      if (gate(resp.status)) {
        return;
      }
      if (resp.ok) {
        setBinding((await resp.json()) as Binding);
      }
    } catch {
      // Durum satırı boş kalır; kod üretme denemesi kendi hatasını gösterir.
    }
  }, [gate]);

  useEffect(() => {
    void refreshBinding();
  }, [refreshBinding]);

  // Kod geri sayımı — süre dolunca kod kaybolur (backend'de de geçersizdir).
  useEffect(() => {
    if (!code) {
      return;
    }
    const tick = () => {
      const left = Math.max(0, Math.ceil((code.expiresAt - Date.now()) / 1000));
      setRemaining(left);
      if (left === 0) {
        setCode(null);
      }
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => window.clearInterval(timer);
  }, [code]);

  const generate = useCallback(async () => {
    setBusy(true);
    setError(null);
    setCopied(false);
    try {
      const resp = await adminFetch("/api/ext/twitch/code", { method: "POST" });
      if (gate(resp.status)) {
        return;
      }
      if (!resp.ok) {
        setError(labels.errors.failed);
        return;
      }
      const body = (await resp.json()) as { code: string; expires_in_secs: number };
      setCode({ code: body.code, expiresAt: Date.now() + body.expires_in_secs * 1000 });
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusy(false);
    }
  }, [gate, labels.errors.failed, labels.errors.network]);

  const unlink = useCallback(async () => {
    if (!window.confirm(labels.confirmUnlink)) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const resp = await adminFetch("/api/ext/twitch/binding", { method: "DELETE" });
      if (gate(resp.status)) {
        return;
      }
      if (resp.ok) {
        setBinding((await resp.json()) as Binding);
      } else {
        setError(labels.errors.failed);
      }
    } catch {
      setError(labels.errors.network);
    } finally {
      setBusy(false);
    }
  }, [gate, labels.confirmUnlink, labels.errors.failed, labels.errors.network]);

  const copy = useCallback(async () => {
    if (!code) {
      return;
    }
    try {
      await navigator.clipboard.writeText(code.code);
      setCopied(true);
    } catch {
      // Pano erişimi reddedilirse kod zaten ekranda okunur durumda.
    }
  }, [code]);

  if (serviceOff) {
    return <p className="text-xs text-muted">{labels.disabledNote}</p>;
  }

  if (!hasSession) {
    return (
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <button
          className="inline-flex h-10 items-center justify-center gap-1.5 rounded-full bg-primary px-4 text-sm font-semibold text-on-primary opacity-50"
          disabled
          type="button"
        >
          <Icon className="h-4 w-4" name="KeyRound" />
          {labels.codeButton}
        </button>
        <p className="text-xs text-muted sm:max-w-md">{labels.signInNote}</p>
      </div>
    );
  }

  const minutes = Math.floor(remaining / 60);
  const seconds = String(remaining % 60).padStart(2, "0");

  return (
    <div className="flex flex-col gap-3">
      {binding ? (
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span
            className={`h-1.5 w-1.5 rounded-full ${binding.linked ? "bg-success" : "bg-faint"}`}
          />
          {binding.linked ? (
            <>
              <span className="text-ink">{labels.linkedLabel}:</span>
              <span className="font-mono text-xs text-ink">{binding.channels.join(", ")}</span>
              <button
                className="inline-flex h-8 items-center rounded-full border border-border bg-surface px-3 text-xs font-semibold text-ink transition hover:border-danger hover:text-danger disabled:opacity-50"
                disabled={busy}
                onClick={unlink}
                type="button"
              >
                {labels.unlinkLabel}
              </button>
            </>
          ) : (
            <span className="text-muted">{labels.notLinkedLabel}</span>
          )}
        </div>
      ) : null}

      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <button
          className="inline-flex h-10 items-center justify-center gap-1.5 rounded-full bg-primary px-4 text-sm font-semibold text-on-primary transition hover:bg-primary-hover disabled:opacity-50"
          disabled={busy}
          onClick={generate}
          type="button"
        >
          <Icon className="h-4 w-4" name="KeyRound" />
          {busy ? labels.generatingLabel : labels.codeButton}
        </button>
        {error ? <p className="text-xs font-medium text-danger">{error}</p> : null}
      </div>

      {code ? (
        <div className="flex flex-wrap items-center gap-3 rounded-lg border border-hairline bg-canvas px-3 py-2">
          <span className="font-mono text-xl font-semibold tracking-[0.3em] text-ink">
            {code.code}
          </span>
          <button
            className="inline-flex h-8 items-center gap-1 rounded-full border border-border bg-surface px-3 text-xs font-semibold text-ink transition hover:bg-canvas"
            onClick={copy}
            type="button"
          >
            <Icon className="h-3.5 w-3.5" name="Copy" />
            {copied ? labels.copiedLabel : labels.copyLabel}
          </button>
          <span className="text-xs text-muted">
            {labels.codeReadyLabel} · {labels.validForLabel}{" "}
            <span className="font-mono">{minutes}:{seconds}</span>
          </span>
        </div>
      ) : null}
    </div>
  );
}
