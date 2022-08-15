/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./pages/**/*.{js,ts,jsx,tsx}",
    "./components/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    colors: {
      transparent: 'transparent',
      current: 'currentColor',
      'blue': '#334675',
      'green': '#2af588',
      'darker-background': '#1d1e21',
      'dark-background': '#26282c',
      'dark-background-accent': '#36393f',
      'light-background': '#e5e7eb',
      'bright': '#e3e3e4',
      'fade': '#767a82',
      'selection-green': '#6dd277',
      'selection-red': '#d26d6d',
    },
    fontFamily: {
      'body': ['Satoshi', 'sans-serif'],
      'heading': ['Chillax', 'sans-serif'],
    },
    fontSize: {
      'small': '0.75rem',
      'base': '1rem',
      'medium': '1.5rem',
      'large': '2rem',
      'xlarge': '3rem',
      '2xlarge': '4rem',
    },
    letterSpacing: {
      'tight': '-0.04em',
      'normal': '0',
      'wide': '0.04em',
    },
  },
  plugins: [],
}
