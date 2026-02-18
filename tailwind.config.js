/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        background: 'rgb(var(--background) / <alpha-value>)',
        foreground: 'rgb(var(--foreground) / <alpha-value>)',
        card: {
          DEFAULT: 'rgb(var(--card) / <alpha-value>)',
          foreground: 'rgb(var(--card-foreground) / <alpha-value>)',
        },
        primary: {
          DEFAULT: 'rgb(var(--primary) / <alpha-value>)',
          foreground: 'rgb(var(--primary-foreground) / <alpha-value>)',
        },
        secondary: {
          DEFAULT: 'rgb(var(--secondary) / <alpha-value>)',
          foreground: 'rgb(var(--secondary-foreground) / <alpha-value>)',
        },
        muted: {
          DEFAULT: 'rgb(var(--muted) / <alpha-value>)',
          foreground: 'rgb(var(--muted-foreground) / <alpha-value>)',
        },
        accent: {
          DEFAULT: 'rgb(var(--accent) / <alpha-value>)',
          foreground: 'rgb(var(--accent-foreground) / <alpha-value>)',
        },
        destructive: {
          DEFAULT: 'rgb(var(--destructive) / <alpha-value>)',
          foreground: 'rgb(var(--destructive-foreground) / <alpha-value>)',
        },
        border: 'var(--border)',
        input: 'var(--input)',
        'input-background': 'rgb(var(--input-background) / <alpha-value>)',
        ring: 'rgb(var(--ring) / <alpha-value>)',
        brand: {
          DEFAULT: 'rgb(var(--brand) / <alpha-value>)',
          foreground: 'rgb(var(--brand-foreground) / <alpha-value>)',
        },
        highlight: {
          DEFAULT: 'rgb(var(--highlight) / <alpha-value>)',
          foreground: 'rgb(var(--highlight-foreground) / <alpha-value>)',
        },
        success: {
          DEFAULT: 'rgb(var(--success) / <alpha-value>)',
          foreground: 'rgb(var(--success-foreground) / <alpha-value>)',
        },
        warning: {
          DEFAULT: 'rgb(var(--warning) / <alpha-value>)',
          foreground: 'rgb(var(--warning-foreground) / <alpha-value>)',
        },
        info: {
          DEFAULT: 'rgb(var(--info) / <alpha-value>)',
          foreground: 'rgb(var(--info-foreground) / <alpha-value>)',
        },
      },
      fontFamily: {
        sans: ['Inter', 'Segoe UI', 'system-ui', 'sans-serif'],
      },
      borderRadius: {
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
      },
      animation: {
        'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        shimmer: 'shimmer 2s ease-in-out infinite',
        countdown: 'countdown-shrink linear forwards',
      },
      keyframes: {
        shimmer: {
          '0%, 100%': { backgroundPosition: '200% 0' },
          '50%': { backgroundPosition: '-200% 0' },
        },
        'countdown-shrink': {
          from: { transform: 'scaleX(1)' },
          to: { transform: 'scaleX(0)' },
        },
      },
      boxShadow: {
        card: '0 1px 2px rgba(0,0,0,0.04), 0 2px 6px rgba(0,0,0,0.03)',
        'card-hover': '0 2px 4px rgba(0,0,0,0.06), 0 4px 12px rgba(0,0,0,0.04)',
        float: '0 4px 12px rgba(0,0,0,0.08), 0 8px 24px rgba(0,0,0,0.06)',
      },
      transitionDuration: {
        fast: '100ms',
        normal: '150ms',
        slow: '300ms',
      },
    },
  },
  plugins: [],
  darkMode: 'class',
}
