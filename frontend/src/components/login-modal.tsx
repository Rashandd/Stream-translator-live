"use client";

import { useState } from "react";
import { LoginForm } from "@/src/components/login-form";
import { OAuthButtons } from "@/src/components/oauth-buttons";
import { VerifyCodeForm } from "@/src/components/verify-code-form";
import type { PublicConfig } from "@/src/lib/config";
import type { AuthNoticeCode } from "@/src/lib/auth-notice";
import type { Locale } from "@/src/i18n/routing";

/**
 * `AuthGuard` modal modunda (bkz. auth-guard.tsx) taşınan yapılandırma —
 * `open`/`noticeCode` guard tarafından enjekte edilir, gerisi layout'tan gelir.
 */
export type LoginModalConfig = {
  locale: Locale;
  labels: PublicConfig["auth"];
  dashboardHref: string;
  adminHref: string;
  forgotHref: string;
  signupHref: string;
};

type Props = LoginModalConfig & {
  open: boolean;
  /** Neden giriş isteniyor — arka planda `AuthNotice` ile aynı etiketler gösterilir. */
  noticeCode: AuthNoticeCode | null;
};

const NOTICE_LABEL_KEY: Record<AuthNoticeCode, keyof PublicConfig["auth"]["errors"]> = {
  sign_in_required: "signInRequired",
  session_expired: "sessionExpired",
  session_unavailable: "sessionUnavailable",
};

/**
 * Yayıncı dashboard'unda oturumsuz erişimde tam sayfa `/login`'e atmak yerine
 * arkadaki kabuğun üstünde açılan giriş penceresi. Başarılı yayıncı girişinde
 * sayfa yeniden yüklenir (cookie zaten set edildi); admin/OAuth/şifre-sıfırlama
 * hâlâ ilgili sayfaya tam navigasyon yapar. 2FA gerekirse aynı pencere içinde
 * doğrulama adımına geçilir. Kapatma/backdrop-click yoktur — arka içerik zaten
 * render edilmez, kullanıcı giriş yapmadan devam edemez (redirect davranışıyla
 * aynı zorunluluk).
 */
export function LoginModal({
  open,
  noticeCode,
  locale,
  labels,
  dashboardHref,
  adminHref,
  forgotHref,
  signupHref,
}: Props) {
  const [step, setStep] = useState<"login" | "verify">("login");

  if (!open) {
    return null;
  }

  const heading = step === "login" ? labels.login : labels.verify;

  return (
    <div
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-emphasis/70 px-4 py-8"
      role="dialog"
    >
      <div className="w-full max-w-md rounded-2xl border border-border bg-surface p-6 sm:p-8">
        <h1 className="text-2xl font-semibold text-ink">{heading.title}</h1>
        <p className="mt-2 text-sm leading-6 text-muted">{heading.subtitle}</p>

        {step === "login" && noticeCode ? (
          <p
            className="mt-4 rounded-lg border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-ink"
            role="status"
          >
            {labels.errors[NOTICE_LABEL_KEY[noticeCode]]}
          </p>
        ) : null}

        {step === "login" ? (
          <>
            <LoginForm
              adminHref={adminHref}
              dashboardHref={dashboardHref}
              forgotHref={forgotHref}
              labels={{
                emailLabel: labels.login.emailLabel,
                passwordLabel: labels.login.passwordLabel,
                forgotLabel: labels.login.forgotLabel,
                submitLabel: labels.login.submitLabel,
                submittingLabel: labels.login.submittingLabel,
                errorInvalid: labels.errors.invalid,
                errorUnavailable: labels.errors.unavailable,
                errorNetwork: labels.errors.network,
              }}
              onAdmin={() => window.location.assign(adminHref)}
              onBroadcaster={() => window.location.reload()}
              onTwoFactor={() => setStep("verify")}
            />

            <OAuthButtons intent="login" labels={labels.social} locale={locale} />

            <p className="mt-6 text-center text-sm text-muted">
              {labels.login.signupPrompt}{" "}
              <a className="font-semibold text-blue hover:text-ink" href={signupHref}>
                {labels.login.signupLabel}
              </a>
            </p>
          </>
        ) : (
          <>
            <VerifyCodeForm
              adminHref={adminHref}
              dashboardHref={dashboardHref}
              labels={{
                codeLabel: labels.verify.codeLabel,
                codePlaceholder: labels.verify.codePlaceholder,
                submitLabel: labels.verify.submitLabel,
                submittingLabel: labels.verify.submittingLabel,
                resendLabel: labels.verify.resendLabel,
                resendingLabel: labels.verify.resendingLabel,
                resendSent: labels.verify.resendSent,
                errorInvalid: labels.errors.codeInvalid,
                errorTooMany: labels.errors.codeTooMany,
                errorNetwork: labels.errors.network,
                errorSessionMissing: labels.errors.sessionMissing,
              }}
              onAdmin={() => window.location.assign(adminHref)}
              onBroadcaster={() => window.location.reload()}
            />

            <div className="mt-4 flex items-center justify-end text-sm">
              <button
                className="font-semibold text-muted hover:text-ink"
                onClick={() => setStep("login")}
                type="button"
              >
                {labels.verify.backLabel}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
