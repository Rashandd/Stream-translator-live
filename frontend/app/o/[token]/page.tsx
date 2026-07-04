"use client";

import { useEffect, useState } from "react";
import { useParams, useSearchParams } from "next/navigation";
import { computeOverlayStyle, OVERLAY_STYLE_DEFAULTS } from "@/src/lib/overlay-style";

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
  const font = search.get("font") ?? OVERLAY_STYLE_DEFAULTS.font;
  const size = search.get("size") ?? OVERLAY_STYLE_DEFAULTS.size;
  const color = search.get("color") ?? OVERLAY_STYLE_DEFAULTS.color;
  const bg = search.get("bg") ?? OVERLAY_STYLE_DEFAULTS.bg;
  const position = search.get("position") ?? OVERLAY_STYLE_DEFAULTS.position;
  const style = computeOverlayStyle({ font, size, color, bg, position });

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
    <div style={style.container}>
      {text ? <span style={style.caption}>{text}</span> : null}
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
