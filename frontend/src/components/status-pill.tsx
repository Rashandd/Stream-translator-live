type StatusPillProps = {
  label: string;
  text: string;
};

const statusClassName: Record<string, string> = {
  enabled: "border-success/25 bg-success/10 text-ink",
  beta: "border-blue/25 bg-blue/10 text-ink",
  waitlist: "border-violet/25 bg-violet/10 text-ink",
  coming_soon: "border-cyan/25 bg-cyan/10 text-ink",
  disabled: "border-border bg-canvas text-muted",
};

export function StatusPill({ label, text }: StatusPillProps) {
  return (
    <span className={`inline-flex rounded-full border px-3 py-1 text-xs font-semibold ${statusClassName[label] ?? statusClassName.disabled}`}>
      {text}
    </span>
  );
}
