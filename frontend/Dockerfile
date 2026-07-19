# StreamDubber frontend — multi-stage build (Next.js standalone).
# Build context: frontend/ dizini (compose: context ../frontend; Railway: root=frontend).
# NEXT_PUBLIC_* değerleri build-arg'dır: tarayıcıya gömülür, bu yüzden build
# anında verilmek zorundadır (runtime env yeterli değil).

FROM node:22-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:22-alpine AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
ARG NEXT_PUBLIC_API_URL=http://localhost:8080
ARG NEXT_PUBLIC_OVERLAY_WS_URL=ws://localhost:8080
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL \
    NEXT_PUBLIC_OVERLAY_WS_URL=$NEXT_PUBLIC_OVERLAY_WS_URL \
    NEXT_TELEMETRY_DISABLED=1
RUN npm run build

# --- Runtime ---
FROM node:22-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production \
    PORT=3000 \
    HOSTNAME=0.0.0.0 \
    NEXT_TELEMETRY_DISABLED=1

RUN addgroup -S nodejs && adduser -S nextjs -G nodejs
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static
COPY --from=builder --chown=nextjs:nodejs /app/public ./public
USER nextjs

EXPOSE 3000

# node'un global fetch'i ile basit sayfa probe'u (alpine'da curl yok).
HEALTHCHECK --interval=30s --timeout=5s --retries=3 --start-period=15s \
    CMD node -e "fetch('http://127.0.0.1:3000/').then((r)=>{if(!r.ok)process.exit(1)}).catch(()=>process.exit(1))"

CMD ["node", "server.js"]
