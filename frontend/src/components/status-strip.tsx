import { Icon } from "@/src/components/icon";

type StatusStripItem = {
  label: string;
  value: string;
  tone: string;
  icon: string;
};

type StatusStripProps = {
  ariaLabel: string;
  items: StatusStripItem[];
};

const toneDot: Record<string, string> = {
  success: "bg-success",
  warning: "bg-warning",
  danger: "bg-danger",
  muted: "bg-faint",
};

export function StatusStrip({ ariaLabel, items }: StatusStripProps) {
  return (
    <div
      aria-label={ariaLabel}
      className="grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-border bg-hairline sm:grid-cols-4"
      role="group"
    >
      {items.map((item) => (
        <div className="flex items-center gap-3 bg-surface px-4 py-3" key={item.label}>
          <Icon className="h-5 w-5 shrink-0 text-muted" name={item.icon} />
          <div className="min-w-0">
            <p className="text-xs font-medium text-muted">{item.label}</p>
            <p className="flex items-center gap-1.5 truncate font-mono text-sm font-semibold text-ink">
              <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${toneDot[item.tone] ?? toneDot.muted}`} />
              {item.value}
            </p>
          </div>
        </div>
      ))}
    </div>
  );
}
