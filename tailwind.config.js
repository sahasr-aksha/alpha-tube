/** @type {import('tailwindcss').Config} */
export default {
    content: [
        "./index.html",
        "./src/**/*.{js,ts,jsx,tsx}",
    ],
    theme: {
        extend: {
            colors: {
                // Dark Theme Colors
                'deep-space': '#0f0f12',
                'card-dark': '#1E1E24',
                'card-darker': '#15151a',
                'glass-border': 'rgba(255, 255, 255, 0.1)',
                'text-main': '#ffffff',
                'text-muted': '#a0a0b0',
                // Accent Colors - Brighter for dark theme
                'electric-lavender': '#B388FF',
                'neo-mint': '#00E5CC',
                'cyber-pink': '#FF4081',
            },
            boxShadow: {
                'soft-glow': '0 8px 32px 0 rgba(0, 0, 0, 0.4)',
                'glass-edge': 'inset 0 0 0 1px rgba(255, 255, 255, 0.1)',
                'neon-glow': '0 0 20px rgba(255, 64, 129, 0.4)',
                'card-float': '0 10px 40px rgba(0, 0, 0, 0.5)',
                'neon-mint': '0 0 20px rgba(0, 229, 204, 0.3)',
            },
            borderRadius: {
                DEFAULT: '1.0rem',
            },
            fontFamily: {
                mono: ['"JetBrains Mono"', 'monospace'],
            }
        },
    },
    plugins: [],
}
