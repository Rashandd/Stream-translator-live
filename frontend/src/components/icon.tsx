import {
  AudioLines,
  Building2,
  CheckCircle2,
  Circle,
  Code2,
  Languages,
  LockKeyhole,
  MessagesSquare,
  MonitorCog,
  MonitorUp,
  PanelsTopLeft,
  Presentation,
  Radio,
  Route,
  ShieldCheck,
  Video,
  type LucideIcon,
} from "lucide-react";

const icons: Record<string, LucideIcon> = {
  AudioLines,
  Building2,
  CheckCircle2,
  Code2,
  Languages,
  LockKeyhole,
  MessagesSquare,
  MonitorCog,
  MonitorUp,
  PanelsTopLeft,
  Presentation,
  Radio,
  Route,
  ShieldCheck,
  Video,
};

type IconProps = {
  name: string;
  className?: string;
  "aria-hidden"?: boolean;
};

export function Icon({ name, className, "aria-hidden": ariaHidden = true }: IconProps) {
  const Component = icons[name] ?? Circle;

  return <Component aria-hidden={ariaHidden} className={className} strokeWidth={1.8} />;
}
