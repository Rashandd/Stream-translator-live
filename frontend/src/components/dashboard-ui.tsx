import type { ReactNode } from "react";
import { Icon } from "@/src/components/icon";

type PageHeaderProps = {
  title: string;
  subtitle: string;
  action?: ReactNode;
};

export function PageHeader({ title, subtitle, action }: PageHeaderProps) {
  return (
    <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
      <div>
        <h1 className="text-3xl font-semibold tracking-normal text-ink">{title}</h1>
        <p className="mt-2 max-w-2xl text-base leading-7 text-muted">{subtitle}</p>
      </div>
      {action ? <div className="shrink-0">{action}</div> : null}
    </div>
  );
}

type MetricCardProps = {
  label: string;
  value: string;
  hint: string;
  icon: string;
};

export function MetricCard({ label, value, hint, icon }: MetricCardProps) {
  return (
    <article className="rounded-xl border border-border bg-surface p-5">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium text-muted">{label}</p>
        <Icon className="h-5 w-5 text-blue" name={icon} />
      </div>
      <p className="mt-3 font-mono text-2xl font-semibold text-ink">{value}</p>
      <p className="mt-2 text-sm leading-6 text-muted">{hint}</p>
    </article>
  );
}

type SectionCardProps = {
  title: string;
  subtitle?: string;
  action?: ReactNode;
  children: ReactNode;
};

export function SectionCard({ title, subtitle, action, children }: SectionCardProps) {
  return (
    <section className="rounded-xl border border-border bg-surface p-5 sm:p-6">
      <div className="mb-5 flex items-start justify-between gap-4">
        <div>
          <h2 className="text-lg font-semibold text-ink">{title}</h2>
          {subtitle ? <p className="mt-1 text-sm text-muted">{subtitle}</p> : null}
        </div>
        {action ? <div className="shrink-0">{action}</div> : null}
      </div>
      {children}
    </section>
  );
}
