/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/pages/**/*.{js,ts,jsx,tsx}",
    "./src/components/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    colors: {
      transparent: 'transparent',
      current: 'currentColor',
      'blue': '#59B2F3',
      'green': {
        DEFAULT: '#62DD76',
        faded: '#61AE32',
        accent: '#2AF588'
      },
      'ochre': {
        DEAFULT: '#EFB440',
        faded: '#AE8B32'
      },
      'red': {
        DEFAULT: '#DD6262',
        faded: '#AE3232'
      },
      'gray': {
        300: '#E3E3E4',
        400: '#A5A5AC',
        500: '#767A82',
        600: '#44464B',
        700: '#36393F',
        800: '#26282C',
        900: '#1D1E21'
      },
      'white': '#FFFFFF',
    },
    fontFamily: {
      'body': ['Satoshi', 'sans-serif'],
      'heading': ['Chillax', 'sans-serif'],
      'title': ['Chillax', 'sans-serif'],
    },
    fontSize: {
      'small': '0.7rem',
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
