type Brand = "youtube" | "twitch" | "kick";

type BrandMarkProps = {
  brand: Brand;
  className?: string;
};

export function brandForPlatform(value: string): Brand | null {
  const normalized = value.toLowerCase();
  if (normalized.includes("youtube") || normalized === "google") return "youtube";
  if (normalized.includes("twitch")) return "twitch";
  if (normalized.includes("kick")) return "kick";
  return null;
}

export function BrandMark({ brand, className = "h-5 w-5" }: BrandMarkProps) {
  if (brand === "youtube") {
    return (
      <svg aria-hidden className={className} viewBox="0 0 24 24" role="img">
        <rect fill="#ff0033" height="15" rx="4" width="21" x="1.5" y="4.5" />
        <path d="M10 8.4 16 12l-6 3.6V8.4Z" fill="white" />
      </svg>
    );
  }

  if (brand === "twitch") {
    return (
      <svg aria-hidden className={className} viewBox="0 0 24 24" role="img">
        <path
          d="M5 4h15v10.5l-4.2 4.2h-3.7L9.6 21H7v-2.3H4V7l1-3Z"
          fill="#9146ff"
        />
        <path d="M8 6h10v7.6L15.6 16H12l-2.5 2.2V16H8V6Z" fill="white" />
        <path d="M11 8.3h1.7v4.8H11V8.3Zm4 0h1.7v4.8H15V8.3Z" fill="#9146ff" />
      </svg>
    );
  }

  return (
    <svg aria-hidden className={className} viewBox="0 0 24 24" role="img">
      <rect fill="#53fc18" height="18" rx="3" width="18" x="3" y="3" />
      <path
        d="M8 7h3.2v3.1h1.4L15 7h3.4l-3.7 5 3.9 5H15l-2.4-3.1h-1.4V17H8V7Z"
        fill="#07130a"
      />
    </svg>
  );
}
