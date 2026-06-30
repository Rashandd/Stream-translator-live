import { redirect } from "next/navigation";
import { defaultLocale } from "@/src/i18n/routing";

export default function RootPage() {
  redirect(`/${defaultLocale}`);
}
