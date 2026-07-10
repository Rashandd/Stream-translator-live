import { NextRequest, NextResponse } from "next/server";
import { buildContentSecurityPolicy } from "@/src/lib/content-security-policy";

export function middleware(request: NextRequest) {
  const nonce = btoa(crypto.randomUUID());
  const policy = buildContentSecurityPolicy(nonce, {
    apiUrl: process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080",
    overlayWebSocketUrl: process.env.NEXT_PUBLIC_OVERLAY_WS_URL ?? "ws://localhost:8080",
    development: process.env.NODE_ENV === "development",
  });

  // Passing the nonce-bearing policy on the request lets Next apply the nonce
  // to its own framework and hydration scripts as well as our explicit script.
  const requestHeaders = new Headers(request.headers);
  requestHeaders.set("x-nonce", nonce);
  requestHeaders.set("Content-Security-Policy", policy);

  const response = NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set("Content-Security-Policy", policy);
  response.headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("Permissions-Policy", "camera=(), geolocation=(), microphone=(self)");
  return response;
}

export const config = {
  matcher: [
    {
      source: "/((?!_next/static|_next/image|favicon.ico).*)",
      missing: [
        { type: "header", key: "next-router-prefetch" },
        { type: "header", key: "purpose", value: "prefetch" },
      ],
    },
  ],
};
