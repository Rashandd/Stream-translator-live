"use client";

import { useEffect, useState, type ReactNode } from "react";
import { LoginModal, type LoginModalConfig } from "@/src/components/login-modal";
import { adminFetch, userFetch } from "@/src/lib/api";
import {
  AUTH_CHANNEL,
  hasSeenSession,
  LOGOUT_EVENT_KEY,
  markSessionSeen,
  storeAuthNotice,
  type AuthNoticeCode,
} from "@/src/lib/auth-notice";

type BaseProps = {
  children: ReactNode;
  /** Hangi oturum kontrol edilecek — yayıncı paneli mi admin konsolu mu. */
  tokenKind: "user" | "admin";
};

/** Varsayılan davranış: oturum yoksa tam sayfa giriş sayfasına yönlendir. */
type RedirectProps = BaseProps & {
  mode?: "redirect";
  /** Oturum yoksa yönlendirilecek giriş sayfası (locale'li). */
  loginHref: string;
};

/** Modal davranışı: oturum yoksa yönlendirmeden arkada login penceresi aç. */
type ModalProps = BaseProps & {
  mode: "modal";
  modalConfig: LoginModalConfig;
};

type Props = RedirectProps | ModalProps;

/**
 * Dashboard/admin layout'larını sarmalar: yetkili bir oturum yoksa içerik
 * hiç render edilmeden giriş sayfasına yönlendirir.
 *
 * Bütün browser auth türleri (şifre, 2FA, OAuth) artık aynı HttpOnly cookie
 * session'ı kullanır (Faz 2); oturum JS'ten görünmez, bu yüzden kontrol her
 * durumda `GET /api/auth/session`'a gerçek bir istek atarak yapılır (backend
 * `AuthUser` extractor'ı çerezi doğrular). localStorage bearer kısayolu
 * kaldırıldı — eski/süresi dolmuş bir token artık geçerli cookie session'ı
 * gölgeleyemez.
 */
export function AuthGuard(props: Props) {
  const { children, tokenKind } = props;
  const isModal = props.mode === "modal";
  const loginHref = props.mode === "modal" ? null : props.loginHref;
  const [authorized, setAuthorized] = useState(false);
  const [noticeCode, setNoticeCode] = useState<AuthNoticeCode | null>(null);

  useEffect(() => {
    let cancelled = false;

    // Modal modda yönlendirme yok: notice'i state'e yazıp arkada login
    // penceresini açarız. Redirect modda (varsayılan) eski davranış korunur.
    function handleUnauthorized(reason: AuthNoticeCode) {
      if (isModal) {
        setAuthorized(false);
        setNoticeCode(reason);
        return;
      }
      storeAuthNotice(reason);
      window.location.replace(loginHref as string);
    }

    async function check() {
      try {
        const fetcher = tokenKind === "admin" ? adminFetch : userFetch;
        const response = await fetcher("/api/auth/session");
        if (cancelled) {
          return;
        }
        if (!response.ok) {
          handleUnauthorized(hasSeenSession() ? "session_expired" : "sign_in_required");
          return;
        }
        const data = (await response.json()) as { authenticated?: boolean; role?: string };
        const roleAllowed = tokenKind === "admin" ? data.role === "admin" : true;
        if (data.authenticated && roleAllowed) {
          markSessionSeen();
          setNoticeCode(null);
          setAuthorized(true);
        } else {
          handleUnauthorized(hasSeenSession() ? "session_expired" : "sign_in_required");
        }
      } catch {
        if (!cancelled) {
          handleUnauthorized("session_unavailable");
        }
      }
    }

    const handleLogout = () => handleUnauthorized("sign_in_required");
    const handleStorage = (event: StorageEvent) => {
      if (event.key === LOGOUT_EVENT_KEY) handleLogout();
    };
    const channel = typeof BroadcastChannel === "undefined" ? null : new BroadcastChannel(AUTH_CHANNEL);
    if (channel) channel.onmessage = handleLogout;
    window.addEventListener("storage", handleStorage);
    void check();
    return () => {
      cancelled = true;
      channel?.close();
      window.removeEventListener("storage", handleStorage);
    };
  }, [tokenKind, loginHref, isModal]);

  if (authorized) {
    return <>{children}</>;
  }
  if (props.mode === "modal" && noticeCode) {
    return <LoginModal noticeCode={noticeCode} open {...props.modalConfig} />;
  }
  return null;
}
