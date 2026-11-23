/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: {
          DEFAULT: "#0f0e13",
          subtle: "#15141b",
          surface: "#1b1a23",
          elevated: "#24222e",
          border: "#302e3d",
        },
        fg: {
          DEFAULT: "#ecebf2",
          muted: "#a3a1b4",
          faint: "#6f6d80",
        },
        accent: {
          DEFAULT: "#f2b84b",
          dim: "#b9862b",
        },
        good: "#6fcf8e",
        bad: "#f07c7c",
      },
      fontFamily: {
        sans: ["Segoe UI Variable Text", "Segoe UI", "system-ui", "-apple-system", "sans-serif"],
        mono: ["Cascadia Mono", "Consolas", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};
