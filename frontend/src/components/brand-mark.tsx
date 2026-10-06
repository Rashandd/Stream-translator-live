type Brand = "google" | "youtube" | "twitch" | "kick";

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
  if (brand === "google") {
    return (
      <svg aria-hidden className={className} viewBox="0 0 24 24" role="img">
        <path
          fill="#4285F4"
          d="M23.5 12.3c0-.8-.1-1.6-.2-2.3H12v4.5h6.5a5.6 5.6 0 0 1-2.4 3.6v3h3.9c2.3-2.1 3.5-5.2 3.5-8.8Z"
        />
        <path
          fill="#34A853"
          d="M12 24c3.2 0 6-1.1 8-2.9l-3.9-3c-1.1.7-2.5 1.2-4.1 1.2-3.1 0-5.8-2.1-6.7-5H1.3v3.1A12 12 0 0 0 12 24Z"
        />
        <path
          fill="#FBBC05"
          d="M5.3 14.3a7.2 7.2 0 0 1 0-4.6V6.6H1.3a12 12 0 0 0 0 10.8l4-3.1Z"
        />
        <path
          fill="#EA4335"
          d="M12 4.8c1.8 0 3.3.6 4.6 1.8l3.4-3.4A12 12 0 0 0 1.3 6.6l4 3.1c.9-2.9 3.6-5 6.7-5Z"
        />
      </svg>
    );
  }

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
