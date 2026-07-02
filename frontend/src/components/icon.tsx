import {
  AudioLines,
  BarChart3,
  Building2,
  CheckCircle2,
  Circle,
  Code2,
  Copy,
  CreditCard,
  Download,
  Gauge,
  KeyRound,
  Languages,
  LayoutDashboard,
  LockKeyhole,
  Mail,
  Menu,
  MessagesSquare,
  Mic,
  MonitorCog,
  MonitorUp,
  PanelsTopLeft,
  Play,
  Plus,
  Presentation,
  Radio,
  Route,
  ScrollText,
  Settings,
  Share2,
  ShieldCheck,
  Subtitles,
  Video,
  Wifi,
  X,
  type LucideIcon,
} from "lucide-react";

const icons: Record<string, LucideIcon> = {
  AudioLines,
  BarChart3,
  Building2,
  CheckCircle2,
  Code2,
  Copy,
  CreditCard,
  Download,
  Gauge,
  KeyRound,
  Languages,
  LayoutDashboard,
  LockKeyhole,
  Mail,
  Menu,
  MessagesSquare,
  Mic,
  MonitorCog,
  MonitorUp,
  PanelsTopLeft,
  Play,
  Plus,
  Presentation,
  Radio,
  Route,
  ScrollText,
  Settings,
  Share2,
  ShieldCheck,
  Subtitles,
  Video,
  Wifi,
  X,
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
