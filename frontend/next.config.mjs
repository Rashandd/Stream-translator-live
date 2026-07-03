/** @type {import('next').NextConfig} */
const nextConfig = {
  // Docker/Railway runtime'ı: server.js + minimal node_modules üreten
  // kendi kendine yeten çıktı (frontend/Dockerfile bunu kopyalar).
  output: "standalone",
  devIndicators: false,
  poweredByHeader: false,
};

export default nextConfig;
