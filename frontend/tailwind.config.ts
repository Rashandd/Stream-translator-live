import type { Config } from "tailwindcss";

const withOpacity = (variable: string) => `rgb(var(${variable}) / <alpha-value>)`;

const config: Config = {
  darkMode: "class",
  content: [
    "./app/**/*.{js,ts,jsx,tsx,mdx}",
    "./src/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  theme: {
    extend: {
      colors: {
        canvas: withOpacity("--color-canvas"),
        surface: withOpacity("--color-surface"),
        ink: withOpacity("--color-ink"),
        text: withOpacity("--color-text"),
        muted: withOpacity("--color-muted"),
        faint: withOpacity("--color-faint"),
        hairline: withOpacity("--color-hairline"),
        border: withOpacity("--color-border"),
        // Brand primary — CTA buttons/badges. Kept independent from the
        // "ink"/"text" heading tokens so dark mode can swap in a legible
        // accent instead of navy-on-near-black.
        primary: withOpacity("--color-primary"),
        "primary-hover": withOpacity("--color-primary-hover"),
        cyan: "#36D8E4",
        blue: "#2F6BFF",
        violet: "#9A8CFF",
        coral: "#FF6A4A",
        success: "#13A66B",
        warning: "#F59E0B",
        danger: "#D83A46",
      },
      boxShadow: {
        panel: "0 24px 80px rgba(6, 26, 61, 0.10)",
      },
      fontFamily: {
        sans: ["var(--font-inter)", "Inter", "system-ui", "sans-serif"],
        mono: ["var(--font-geist-mono)", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};

export default config;
