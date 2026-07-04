/** @type {import('next').NextConfig} */
const nextConfig = {
  // Lokal Docker runtime'ı için: server.js + minimal node_modules üreten
  // kendi kendine yeten çıktı (frontend/Dockerfile kopyalar). Üretim deploy
  // Vercel'dedir ve bu ayarı yok sayar (bkz. infra/deploy.md).
  output: "standalone",
  devIndicators: false,
  poweredByHeader: false,
};

export default nextConfig;
