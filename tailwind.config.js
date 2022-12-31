// eslint-disable-next-line @typescript-eslint/no-var-requires
const defaultTheme = require('tailwindcss/defaultTheme');
/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './src/app/**/*.{js,ts,jsx,tsx}',
    './src/pages/**/*.{js,ts,jsx,tsx}',
    './src/components/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    colors: {
      transparent: 'transparent',
      current: 'currentColor',
      blue: {
        200: '#59B2F3',
        300: '#1D8EB2',
        DEFAULT: '#1D8EB2',
        400: '#037AA0',
        500: '#13668A',
        600: '#334675',
        faded: '#3C85BA',
      },
      green: {
        200: '#2AF588',
        300: '#6DD277',
        DEFAULT: '#6DD277',
        400: '#3DAE5E',
        faded: '#61AE32',
        enabled: '#48F077'
      },
      red: {
        200: '#FF5C5C',
        300: '#CF4545',
        DEFAULT: '#CF4545',
        400: '#B63737',
        faded: '#AE3232',
      },
      yellow: {
        300: '#EFB440',
        DEFAULT: '#EFB440',
        faded: '#AE8B32',
      },
      gray: {
        300: '#E3E3E4',
        400: '#A5A5AC',
        500: '#767A82',
        550: '#67686A',
        600: '#44464B',
        700: '#36393F',
        750: '#303338',
        800: '#2B2D32',
        850: '#26282C',
        900: '#1D1E21',
        faded: '#A5A5AC',
      },
      white: '#FFFFFF',
      violet: '#8736C7',
      ultramarine: '#273EB9',
    },
    fontFamily: {
      sans: ['Satoshi', ...defaultTheme.fontFamily.sans],
      heading: ['Chillax', ...defaultTheme.fontFamily.sans],
      title: ['Clash Grotesk', ...defaultTheme.fontFamily.sans],
      minecraft: ['Minecraft', ...defaultTheme.fontFamily.sans],
      mono: ['JetBrains Mono', ...defaultTheme.fontFamily.mono],
    },
    fontSize: {
      smaller: '0.625rem',
      small: '0.75rem',
      base: '0.875rem',
      medium: '1rem',
      large: '1.25rem',
      larger: '1.5rem',
      xlarge: '1.75rem',
      '2xlarge': '2rem',
    },
    letterSpacing: {
      tight: '-0.04em',
      medium: '-0.02em',
      normal: '0',
      wide: '0.04em',
    },
    dropShadow: {
      'sm': '0 0 transparent',
      'md': '0 3px 6px #111114',
      'lg': '0 8px 24px #111114',
      'xl': '0 12px 48px #111114',
    },
    fontWeight: {
      'medium-semi-bold': 550,
      ...defaultTheme.fontWeight,
    },
    extend: {
      transitionProperty: {
        'height': 'height',
        'width': 'width',
        'spacing': 'margin, padding',
        'dimensions': 'height, width',
      },
    },
  },
  plugins: [
    function ({ addVariant }) {
      addVariant('child', '& > *');
    },
    require('@tailwindcss/container-queries'),
    require('@headlessui/tailwindcss')({ prefix: 'ui' })
  ],
};
