"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "@/src/components/icon";
import { userFetch } from "@/src/lib/api";

type LanguageMode = "single_to_single" | "multi_to_single" | "single_to_multi";

/** `GET/POST /api/voice/...` model kaydı (backend voice_models::VoiceModel). */
type VoiceModel = {
  id: string;
  name: string;
  status: "processing" | "pending_review" | "approved" | "rejected";
  created_at: number;
  language_mode: LanguageMode;
  source_languages: string[];
  target_languages: string[];
};

/** `GET /api/translate/catalog` kaydı (auth gerekmez, gizli veri yok). */
type LanguageCatalogEntry = {
  code: string;
  name: string;
  supports_formality: boolean;
};

type LanguageModeOption = { title: string; description: string };

type FormLabels = {
  nameLabel: string;
  namePlaceholder: string;
  recordLabel: string;
  stopLabel: string;
  recordingNote: string;
  uploadLabel: string;
  sampleReadyTemplate: string;
  sampleReadyGeneric: string;
  submitLabel: string;
  submittingLabel: string;
  micDeniedError: string;
  noSampleError: string;
  languageModeLabel: string;
  languageModeOptions: Record<LanguageMode, LanguageModeOption>;
  sourceLanguageLabel: string;
  sourceLanguagesLabel: string;
  sourceLanguagePlaceholder: string;
  noSourceLanguageError: string;
  notEnoughSourceLanguagesError: string;
  consentLabel: string;
  noConsentError: string;
};

type ModelMetaLabels = {
  languageModeLabels: Record<LanguageMode, string>;
  sourceLabel: string;
  targetLabel: string;
  targetUnassigned: string;
};

type Labels = {
  form: FormLabels;
  modelsTitle: string;
  modelsEmpty: string;
  statusLabels: Record<VoiceModel["status"], string>;
  modelMeta: ModelMetaLabels;
  errors: {
    unauthorized: string;
    network: string;
    planLocked: string;
    providerDisabled: string;
    failed: string;
  };
};

function fill(template: string, values: Record<string, string>): string {
  return Object.entries(values).reduce(
    (acc, [key, value]) => acc.replaceAll(`{${key}}`, value),
    template,
  );
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onloadend = () => {
      const result = typeof reader.result === "string" ? reader.result : "";
      resolve(result.split(",")[1] ?? "");
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

const statusTone: Record<VoiceModel["status"], string> = {
  processing: "border-border bg-canvas text-muted",
  pending_review: "border-warning/25 bg-warning/10 text-ink",
  approved: "border-success/25 bg-success/10 text-ink",
  rejected: "border-danger/25 bg-danger/10 text-ink",
};

const LANGUAGE_MODES: LanguageMode[] = ["single_to_multi", "single_to_single", "multi_to_single"];

/** Version of `labels.form.consentLabel` sent as `consent_text_version` —
 * bump this alongside the copy so past acceptances stay tied to the wording
 * they actually agreed to (backend voice_models::VoiceConsent). */
const CONSENT_TEXT_VERSION = "v1";

export function VoiceCloneStudio({ labels }: { labels: Labels }) {
  const [name, setName] = useState("");
  const [sample, setSample] = useState<Blob | null>(null);
  const [recording, setRecording] = useState(false);
  const [recordedSeconds, setRecordedSeconds] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [models, setModels] = useState<VoiceModel[] | null>(null);
  const [catalog, setCatalog] = useState<LanguageCatalogEntry[]>([]);
  const [languageMode, setLanguageMode] = useState<LanguageMode>("single_to_multi");
  const [sourceLangs, setSourceLangs] = useState<string[]>([]);
  const [ownVoiceConsent, setOwnVoiceConsent] = useState(false);

  const recorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const recordStartRef = useRef(0);

  const loadModels = useCallback(async () => {
    try {
      const res = await userFetch("/api/voice/models");
      if (!res.ok) return;
      setModels((await res.json()) as VoiceModel[]);
    } catch {
      // Sessizce yut — sayfa formu kullanılabilir kalır, liste sadece boş görünür.
    }
  }, []);

  const loadCatalog = useCallback(async () => {
    try {
      const res = await userFetch("/api/translate/catalog");
      if (!res.ok) return;
      setCatalog((await res.json()) as LanguageCatalogEntry[]);
    } catch {
      // Sessizce yut — dil seçici boş kalır, kullanıcı yine de kayıt/yükleme yapabilir.
    }
  }, []);

  useEffect(() => {
    void loadModels();
    void loadCatalog();
  }, [loadModels, loadCatalog]);

  const needsMultipleSourceLanguages = languageMode === "multi_to_single";

  const selectSingleSourceLang = useCallback((code: string) => {
    setSourceLangs(code ? [code] : []);
  }, []);

  const toggleSourceLang = useCallback((code: string) => {
    setSourceLangs((prev) =>
      prev.includes(code) ? prev.filter((c) => c !== code) : [...prev, code],
    );
  }, []);

  const changeLanguageMode = useCallback((mode: LanguageMode) => {
    setLanguageMode(mode);
    setSourceLangs([]);
  }, []);

  const startRecording = useCallback(async () => {
    setError(null);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const recorder = new MediaRecorder(stream);
      chunksRef.current = [];
      recorder.ondataavailable = (event) => chunksRef.current.push(event.data);
      recorder.onstop = () => {
        setSample(new Blob(chunksRef.current, { type: "audio/webm" }));
        setRecordedSeconds(Math.round((Date.now() - recordStartRef.current) / 1000));
        stream.getTracks().forEach((track) => track.stop());
      };
      recordStartRef.current = Date.now();
      recorder.start();
      recorderRef.current = recorder;
      setRecording(true);
    } catch {
      setError(labels.form.micDeniedError);
    }
  }, [labels.form.micDeniedError]);

  const stopRecording = useCallback(() => {
    recorderRef.current?.stop();
    setRecording(false);
  }, []);

  const onFileChosen = useCallback((file: File | undefined) => {
    if (!file) return;
    setSample(file);
    setRecordedSeconds(0);
  }, []);

  const submit = useCallback(async () => {
    setError(null);
    if (!sample) {
      setError(labels.form.noSampleError);
      return;
    }
    if (sourceLangs.length === 0) {
      setError(labels.form.noSourceLanguageError);
      return;
    }
    if (needsMultipleSourceLanguages && sourceLangs.length < 2) {
      setError(labels.form.notEnoughSourceLanguagesError);
      return;
    }
    if (!ownVoiceConsent) {
      setError(labels.form.noConsentError);
      return;
    }
    setSubmitting(true);
    try {
      const sampleBase64 = await blobToBase64(sample);
      const res = await userFetch("/api/voice/clone", {
        method: "POST",
        body: JSON.stringify({
          name: name.trim() || "Untitled voice",
          sample_base64: sampleBase64,
          language_mode: languageMode,
          source_languages: sourceLangs,
          consent_text_version: CONSENT_TEXT_VERSION,
          is_own_voice: ownVoiceConsent,
        }),
      });
      if (res.status === 401) {
        setError(labels.errors.unauthorized);
      } else if (res.status === 403) {
        setError(labels.errors.planLocked);
      } else if (res.status === 502) {
        setError(labels.errors.providerDisabled);
      } else if (!res.ok) {
        setError(labels.errors.failed);
      } else {
        setName("");
        setSample(null);
        setRecordedSeconds(0);
        setSourceLangs([]);
        setOwnVoiceConsent(false);
        await loadModels();
      }
    } catch {
      setError(labels.errors.network);
    } finally {
      setSubmitting(false);
    }
  }, [
    labels.errors,
    labels.form.noConsentError,
    labels.form.noSampleError,
    labels.form.noSourceLanguageError,
    labels.form.notEnoughSourceLanguagesError,
    languageMode,
    loadModels,
    name,
    needsMultipleSourceLanguages,
    ownVoiceConsent,
    sample,
    sourceLangs,
  ]);

  const languageNameByCode = useMemo(
    () => new Map(catalog.map((entry) => [entry.code, entry.name])),
    [catalog],
  );

  return (
    <div className="space-y-6">
      <section className="space-y-4 rounded-xl border border-border bg-white p-5 sm:p-6">
        {error ? (
          <p className="rounded-lg border border-danger/25 bg-danger/10 px-4 py-3 text-sm text-ink" role="alert">
            {error}
          </p>
        ) : null}

        <div>
          <label className="text-xs font-medium text-muted" htmlFor="voice-name">
            {labels.form.nameLabel}
          </label>
          <input
            className="mt-1 h-10 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue placeholder:text-faint"
            id="voice-name"
            onChange={(event) => setName(event.target.value)}
            placeholder={labels.form.namePlaceholder}
            type="text"
            value={name}
          />
        </div>

        <div>
          <p className="text-xs font-medium text-muted">{labels.form.languageModeLabel}</p>
          <div className="mt-2 grid gap-2 sm:grid-cols-3">
            {LANGUAGE_MODES.map((mode) => {
              const option = labels.form.languageModeOptions[mode];
              const active = languageMode === mode;
              return (
                <button
                  className={`rounded-lg border px-3 py-2 text-left text-xs transition ${
                    active
                      ? "border-blue bg-blue/5 text-ink"
                      : "border-border text-muted hover:border-blue hover:text-ink"
                  }`}
                  key={mode}
                  onClick={() => changeLanguageMode(mode)}
                  type="button"
                >
                  <span className="block font-semibold">{option.title}</span>
                  <span className="mt-0.5 block leading-5">{option.description}</span>
                </button>
              );
            })}
          </div>
        </div>

        {needsMultipleSourceLanguages ? (
          <div>
            <p className="text-xs font-medium text-muted">{labels.form.sourceLanguagesLabel}</p>
            <ul className="mt-2 flex flex-wrap gap-2">
              {catalog.map((entry) => {
                const checked = sourceLangs.includes(entry.code);
                return (
                  <li key={entry.code}>
                    <label
                      className={`inline-flex cursor-pointer items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-semibold transition ${
                        checked ? "border-blue bg-blue/5 text-ink" : "border-border text-muted"
                      }`}
                    >
                      <input
                        checked={checked}
                        onChange={() => toggleSourceLang(entry.code)}
                        type="checkbox"
                      />
                      {entry.name}
                    </label>
                  </li>
                );
              })}
            </ul>
          </div>
        ) : (
          <div>
            <label className="text-xs font-medium text-muted" htmlFor="voice-source-lang">
              {labels.form.sourceLanguageLabel}
            </label>
            <select
              className="mt-1 h-10 w-full rounded-lg border border-border bg-white px-3 text-sm text-ink outline-none transition focus:border-blue"
              id="voice-source-lang"
              onChange={(event) => selectSingleSourceLang(event.target.value)}
              value={sourceLangs[0] ?? ""}
            >
              <option value="">{labels.form.sourceLanguagePlaceholder}</option>
              {catalog.map((entry) => (
                <option key={entry.code} value={entry.code}>
                  {entry.name}
                </option>
              ))}
            </select>
          </div>
        )}

        <div className="flex flex-wrap items-center gap-3">
          <button
            className={`inline-flex h-10 items-center gap-2 rounded-full px-4 text-sm font-semibold transition ${
              recording
                ? "bg-danger text-white hover:bg-danger/90"
                : "border border-border text-ink hover:border-blue hover:text-blue"
            }`}
            onClick={() => void (recording ? stopRecording() : startRecording())}
            type="button"
          >
            <Icon className="h-4 w-4" name="Mic" />
            {recording ? labels.form.stopLabel : labels.form.recordLabel}
          </button>

          <label className="inline-flex h-10 cursor-pointer items-center gap-2 rounded-full border border-border px-4 text-sm font-semibold text-ink transition hover:border-blue hover:text-blue">
            <Icon className="h-4 w-4" name="Upload" />
            {labels.form.uploadLabel}
            <input
              accept="audio/*"
              className="hidden"
              onChange={(event) => onFileChosen(event.target.files?.[0])}
              type="file"
            />
          </label>
        </div>

        {recording ? <p className="text-xs text-muted">{labels.form.recordingNote}</p> : null}
        {sample && !recording ? (
          <p className="text-xs text-muted">
            {recordedSeconds > 0
              ? fill(labels.form.sampleReadyTemplate, { seconds: String(recordedSeconds) })
              : labels.form.sampleReadyGeneric}
          </p>
        ) : null}

        <label className="flex items-start gap-2.5 text-xs leading-5 text-muted">
          <input
            checked={ownVoiceConsent}
            className="mt-0.5 h-4 w-4 shrink-0"
            onChange={(event) => setOwnVoiceConsent(event.target.checked)}
            type="checkbox"
          />
          {labels.form.consentLabel}
        </label>

        <button
          className="inline-flex h-10 items-center justify-center rounded-full bg-ink px-5 text-sm font-semibold text-white transition hover:bg-text disabled:cursor-not-allowed disabled:opacity-50"
          disabled={submitting || recording}
          onClick={() => void submit()}
          type="button"
        >
          {submitting ? labels.form.submittingLabel : labels.form.submitLabel}
        </button>
      </section>

      <section className="rounded-xl border border-border bg-white p-5 sm:p-6">
        <h2 className="text-lg font-semibold text-ink">{labels.modelsTitle}</h2>
        {!models || models.length === 0 ? (
          <p className="mt-2 text-sm text-muted">{labels.modelsEmpty}</p>
        ) : (
          <ul className="mt-4 space-y-2">
            {models.map((model) => (
              <li
                className="flex items-center justify-between rounded-lg border border-hairline bg-canvas px-4 py-3"
                key={model.id}
              >
                <div>
                  <span className="text-sm font-semibold text-ink">{model.name}</span>
                  <p className="mt-0.5 text-xs text-muted">
                    {labels.modelMeta.languageModeLabels[model.language_mode]}
                    {" · "}
                    {labels.modelMeta.sourceLabel}{" "}
                    {model.source_languages.map((code) => languageNameByCode.get(code) ?? code).join(", ")}
                    {" · "}
                    {labels.modelMeta.targetLabel}{" "}
                    {model.target_languages.length > 0
                      ? model.target_languages.map((code) => languageNameByCode.get(code) ?? code).join(", ")
                      : labels.modelMeta.targetUnassigned}
                  </p>
                </div>
                <span
                  className={`inline-flex rounded-full border px-2.5 py-1 text-xs font-semibold ${statusTone[model.status]}`}
                >
                  {labels.statusLabels[model.status]}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
