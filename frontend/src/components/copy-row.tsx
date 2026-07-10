"use client";

import { useState } from "react";
import { Icon } from "@/src/components/icon";

export function CopyRow({
  value,
  copyLabel,
  copiedLabel,
  className = "",
}: {
  value: string;
  copyLabel: string;
  copiedLabel?: string;
  className?: string;
}) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API kullanılamıyor (izin/HTTPS yok) — sessizce yut, buton
      // en azından değeri seçilebilir/kopyalanabilir metin olarak gösteriyor.
    }
  }

  return (
    <div className={`flex min-w-0 items-center gap-2 rounded-lg border border-border bg-canvas px-3 py-2 ${className}`}>
      <code className="min-w-0 flex-1 truncate font-mono text-sm text-ink">{value}</code>
      <button
        className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border bg-surface px-2.5 py-1 text-xs font-semibold text-ink transition hover:border-blue hover:text-blue"
        onClick={() => void copy()}
        type="button"
      >
        <Icon className="h-3.5 w-3.5" name={copied ? "CheckCircle2" : "Copy"} />
        {copied ? (copiedLabel ?? copyLabel) : copyLabel}
      </button>
    </div>
  );
}
