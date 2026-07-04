import { Icon } from "@/src/components/icon";

export function CopyRow({
  value,
  copyLabel,
  className = "",
}: {
  value: string;
  copyLabel: string;
  className?: string;
}) {
  return (
    <div className={`flex min-w-0 items-center gap-2 rounded-lg border border-border bg-canvas px-3 py-2 ${className}`}>
      <code className="min-w-0 flex-1 truncate font-mono text-sm text-ink">{value}</code>
      <span className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border bg-white px-2.5 py-1 text-xs font-semibold text-ink">
        <Icon className="h-3.5 w-3.5" name="Copy" />
        {copyLabel}
      </span>
    </div>
  );
}
