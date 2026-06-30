import { notFound } from "next/navigation";
import { Icon } from "@/src/components/icon";
import { PageHeader, SectionCard } from "@/src/components/dashboard-ui";
import { StatusPill } from "@/src/components/status-pill";
import { getPublicConfig } from "@/src/lib/config";
import { isLocale, locales, type Locale } from "@/src/i18n/routing";

type Props = { params: Promise<{ locale: string }> };

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export default async function AdminMultistreamPage({ params }: Props) {
  const { locale: rawLocale } = await params;
  if (!isLocale(rawLocale)) {
    notFound();
  }
  const locale: Locale = rawLocale;
  const config = await getPublicConfig(locale);
  const { multistream } = config.admin;

  return (
    <div className="space-y-6">
      <PageHeader subtitle={multistream.subtitle} title={multistream.title} />

      <SectionCard title={multistream.methodsTitle}>
        <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {multistream.methods.map((method) => (
            <li className="rounded-xl border border-border bg-canvas p-4" key={method.name}>
              <div className="flex items-center justify-between">
                <p className="text-sm font-semibold text-ink">{method.name}</p>
                <StatusPill label={method.status} text={config.statusLabels[method.status as keyof typeof config.statusLabels]} />
              </div>
              <p className="mt-3 font-mono text-2xl font-semibold text-ink">{method.count}</p>
            </li>
          ))}
        </ul>
      </SectionCard>

      <SectionCard title={multistream.matrixTitle}>
        <div className="overflow-x-auto">
          <table className="w-full min-w-[420px] text-left text-sm">
            <thead>
              <tr className="border-b border-hairline text-xs uppercase tracking-wide text-faint">
                {multistream.matrixColumns.map((column) => (
                  <th className="px-4 py-3 font-semibold" key={column}>{column}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {multistream.matrix.map((row) => (
                <tr className="border-b border-hairline last:border-0" key={row.method}>
                  <td className="px-4 py-3 font-semibold text-ink">{row.method}</td>
                  <td className="px-4 py-3">
                    <Icon className={`h-4 w-4 ${row.free ? "text-success" : "text-faint"}`} name={row.free ? "CheckCircle2" : "Circle"} />
                  </td>
                  <td className="px-4 py-3">
                    <Icon className={`h-4 w-4 ${row.pro ? "text-success" : "text-faint"}`} name={row.pro ? "CheckCircle2" : "Circle"} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </SectionCard>
    </div>
  );
}
