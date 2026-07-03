import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { AdminLoginForm } from "@/src/components/admin-login-form";
import { Icon } from "@/src/components/icon";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, localizePath, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function AdminLoginPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { login } = config.admin;

  return (
    <main className="flex min-h-screen items-center justify-center bg-canvas px-4 py-12">
      <div className="w-full max-w-md">
        <Link className="mb-8 flex justify-center" href={localizePath(locale, "/")}>
          <Image
            alt={config.brand.name}
            className="h-14 w-auto object-contain"
            height={545}
            priority
            src={config.brand.logoAsset}
            width={2253}
          />
        </Link>

        <div className="rounded-2xl border border-border bg-white p-6 sm:p-8">
          <span className="inline-flex rounded-full border border-violet/30 bg-violet/10 px-2.5 py-0.5 text-xs font-semibold text-ink">
            {config.admin.topbar.roleLabel}
          </span>
          <h1 className="mt-3 text-2xl font-semibold text-ink">{login.title}</h1>
          <p className="mt-2 text-sm leading-6 text-muted">{login.subtitle}</p>

          <AdminLoginForm
            labels={{
              emailLabel: login.emailLabel,
              emailPlaceholder: login.emailPlaceholder,
              passwordLabel: login.passwordLabel,
              passwordPlaceholder: login.passwordPlaceholder,
              submitLabel: login.submitLabel,
              submittingLabel: login.submittingLabel,
              errorInvalid: login.errorInvalid,
              errorUnavailable: login.errorUnavailable,
              errorNetwork: login.errorNetwork,
            }}
            redirectTo={localizePath(locale, "/admin")}
          />

          <p className="mt-4 flex items-center gap-2 text-sm leading-6 text-muted">
            <Icon className="h-4 w-4 shrink-0 text-blue" name="ShieldCheck" />
            {login.twofaNote}
          </p>
        </div>
      </div>
    </main>
  );
}
