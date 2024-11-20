/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: [
    "./src/**/*.{js,jsx,ts,tsx}",
    "./index.html",
  ],
  theme: {
    extend: {
      spacing: {
        '8': '8px',
        '10': '10px',
        '16': '16px',
      },
      margin: {
        '10': '10px',
        '50': '50px'
      },
      backgroundImage: {
        'card-gradient': 'linear-gradient(359deg, rgba(255, 255, 255, 0.90) 9.96%, rgba(226, 245, 251, 0.90) 95.35%);',
        'window-gradient': 'linear-gradient(90deg, rgba(255, 255, 255, 0.55) 0%, rgba(246, 247, 255, 0.55) 100%);'
      },
      fontSize: {
        14: '14px'
      },
      colors: {
        background: 'var(--background)',
        foreground: 'var(--foreground)',
        tab: '#E9F7FA',

        'splash-pills': 'rgba(255, 255, 255, 0.60)',
        'splash-pills-text': 'rgba(0, 0, 0, 0.60)',

        'tool-bold': 'rgba(0, 0, 0, 0.65);',
        'tool': 'rgba(0, 0, 0, 0.50);',

        'tool-result-green': '#028E00;',
        'tool-card': 'rgba(255, 255, 255, 0.80);',
        'user-bubble': 'rgba(85, 95, 231, 0.90);',
        'goose-bubble': 'rgba(0, 0, 0, 0.12);'
      },
      borderRadius: {
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)'
      }
    }
  },
  plugins: [require("tailwindcss-animate")],
}