import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "./app/**/*.{js,ts,jsx,tsx,mdx}",
    "./src/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  theme: {
    extend: {
      colors: {
        canvas: "#F7F8FA",
        surface: "#FFFFFF",
        ink: "#061A3D",
        text: "#273247",
        muted: "#68748A",
        faint: "#98A2B3",
        hairline: "#E8EEF5",
        border: "#D7E0EA",
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
