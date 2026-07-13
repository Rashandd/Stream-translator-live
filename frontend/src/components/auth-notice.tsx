"use client";

import { useEffect, useState } from "react";
import { consumeAuthNotice, type AuthNoticeCode } from "@/src/lib/auth-notice";

type Props = {
  labels: Record<AuthNoticeCode, string>;
};

export function AuthNotice({ labels }: Props) {
  const [notice, setNotice] = useState<AuthNoticeCode | null>(null);

  useEffect(() => {
    setNotice(consumeAuthNotice());
  }, []);

  if (!notice) {
    return null;
  }
  return (
    <p className="mt-4 rounded-lg border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-ink" role="status">
      {labels[notice]}
    </p>
  );
}
