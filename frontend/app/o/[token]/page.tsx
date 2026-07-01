"use client";

import { useEffect, useState } from "react";
import { useParams, useSearchParams } from "next/navigation";

type SubtitleTranslation = { lang: string; text: string };

type SubtitleMessage = {
  type: string;
  source?: { lang: string; text: string; is_final: boolean };
  translations?: SubtitleTranslation[];
};

const DEFAULT_WS_BASE =
  process.env.NEXT_PUBLIC_OVERLAY_WS_URL ?? "ws://localhost:8080";
const RECONNECT_DELAY_MS = 2000;

/**
 * OBS Browser Source altyazı overlay'i (Faz 1.9).
 *
 * `token` backend'in `stream_ack.overlay_token`'ıdır — kendisi bir
 * capability, ek bir login gerekmez. Görünüm `?lang=&font=&size=&color=&bg=&position=`
 * query parametreleriyle özelleştirilir (bkz. dashboard "Caption appearance").
 */
export default function OverlayPage() {
  const params = useParams<{ token: string }>();
  const search = useSearchParams();
  const [subtitle, setSubtitle] = useState<SubtitleMessage | null>(null);

  const lang = search.get("lang");
  const font = search.get("font") ?? "Inter, system-ui, sans-serif";
  const size = search.get("size") ?? "32px";
  const color = search.get("color") ?? "#ffffff";
  const bg = search.get("bg") ?? "rgba(0,0,0,0.6)";
  const position = search.get("position") === "top" ? "flex-start" : "flex-end";

  useEffect(() => {
    // OBS Browser Source video üstüne bindiği için sayfa arka planı
    // şeffaf olmalı — global body arka planını burada override ediyoruz.
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, []);

  useEffect(() => {
    const token = params?.token;
    if (!token) return;

    let socket: WebSocket | null = null;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;

    function connect() {
      socket = new WebSocket(`${DEFAULT_WS_BASE}/overlay/${token}`);
      socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data) as SubtitleMessage;
          if (data.type === "subtitle") {
            setSubtitle(data);
          }
        } catch {
          // Beklenmeyen/parse edilemeyen frame'i yoksay.
        }
      };
      socket.onclose = () => {
        if (!stopped) {
          retryTimer = setTimeout(connect, RECONNECT_DELAY_MS);
        }
      };
    }

    connect();
    return () => {
      stopped = true;
      if (retryTimer) clearTimeout(retryTimer);
      socket?.close();
    };
  }, [params?.token]);

  const text = pickText(subtitle, lang);

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        display: "flex",
        alignItems: position,
        justifyContent: "center",
        padding: "5vh 4vw",
        background: "transparent",
        pointerEvents: "none",
      }}
    >
      {text ? (
        <span
          style={{
            fontFamily: font,
            fontSize: size,
            color,
            background: bg,
            padding: "0.4em 0.8em",
            borderRadius: "0.5em",
            maxWidth: "90vw",
            textAlign: "center",
            lineHeight: 1.3,
          }}
        >
          {text}
        </span>
      ) : null}
    </div>
  );
}

/** `lang` verilmişse eşleşen çeviriyi, yoksa kaynak metni döndürür. */
function pickText(subtitle: SubtitleMessage | null, lang: string | null): string {
  if (!subtitle) return "";
  if (lang) {
    const match = subtitle.translations?.find(
      (t) => t.lang.toLowerCase() === lang.toLowerCase(),
    );
    if (match) return match.text;
  }
  return subtitle.source?.text ?? "";
}
