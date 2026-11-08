const defaultTheme = require('tailwindcss/defaultTheme');
// Loaded from src/globals.css with `@config` (Tailwind CSS v4). The sources to
// scan for class names are listed there with `@source`.
/** @type {import('tailwindcss').Config} */
module.exports = {
  theme: {
    // The colours that change with the theme are CSS variables, set per theme in
    // src/globals.css (`:root` is the dark theme, `[data-theme='light']` the
    // light one). Tailwind's opacity modifiers (`bg-gray-800/50`) still work:
    // Tailwind v4 mixes the variable with transparent (`color-mix`).
    colors: {
      transparent: 'transparent', //! what
      current: 'currentColor',
      blue: {
        150: 'var(--blue-150)',
        200: 'var(--blue-200)',
        300: 'var(--blue-300)',
        DEFAULT: 'var(--blue-300)',
        400: 'var(--blue-400)',
        500: '#13668A',
        600: '#334675',
        faded: '#3C85BA',
      },
      green: {
        200: 'var(--green-200)',
        300: 'var(--green-300)',
        DEFAULT: 'var(--green-300)',
        400: '#3DAE5E',
        faded: '#61AE32',
        enabled: '#48F077',
      },
      red: {
        200: 'var(--red-200)',
        300: '#CF4545',
        DEFAULT: 'var(--red)',
        400: '#B63737',
        faded: '#AE3232',
      },
      yellow: {
        300: 'var(--yellow-300)',
        DEFAULT: 'var(--yellow-300)',
        faded: '#AE8B32',
      },
      // Surfaces (600-900) and text (300-550). The light theme turns the scale
      // around: text becomes dark, surfaces become light.
      gray: {
        300: 'var(--gray-300)',
        400: 'var(--gray-400)',
        500: 'var(--gray-500)',
        550: 'var(--gray-550)',
        600: 'var(--gray-600)',
        700: 'var(--gray-700)',
        750: 'var(--gray-750)',
        800: 'var(--gray-800)',
        850: 'var(--gray-850)',
        875: 'var(--gray-875)',
        900: 'var(--gray-900)',
        faded: 'var(--gray-faded)',
      },
      fade: {
        700: 'var(--fade-700)',
      },
      // The strongest text colour: white in the dark theme, near-black ink in
      // the light theme, so `text-white/50` is muted text in both
      white: 'var(--ink)',
      // Text and knobs on coloured (accent) backgrounds, the same in both themes
      'on-accent': {
        DEFAULT: '#FFFFFF',
        dark: '#1D1E21',
      },
      // Dims the page behind dialogs (the opacity is part of the colour)
      scrim: {
        DEFAULT: 'var(--scrim)',
        strong: 'var(--scrim-strong)',
      },
      // Light-theme levels that src/light-overrides.css maps the opacity
      // classes to (`text-white/50` is muted ink, `border-gray-faded/30` a
      // line, ...). Defined in the dark theme too, as the colours they replace.
      ink: {
        DEFAULT: 'var(--ink)',
        secondary: 'var(--ink-secondary)',
        muted: 'var(--ink-muted)',
        faint: 'var(--ink-faint)',
      },
      line: {
        DEFAULT: 'var(--line)',
        subtle: 'var(--line-subtle)',
        strong: 'var(--line-strong)',
        input: 'var(--line-input)',
        hover: 'var(--line-hover)',
      },
      fill: {
        neutral: 'var(--fill-neutral)',
        'neutral-strong': 'var(--fill-neutral-strong)',
        selected: 'var(--fill-selected)',
      },
      // Status and info pill backgrounds
      pill: {
        green: 'var(--pill-green)',
        red: 'var(--pill-red)',
        yellow: 'var(--pill-yellow)',
        blue: 'var(--pill-blue)',
      },
      focus: {
        DEFAULT: 'var(--focus-ring)',
        danger: 'var(--focus-ring-danger)',
      },
      violet: '#8736C7',
      ultramarine: '#273EB9',
    },
    fontFamily: {
      sans: ['Satoshi', ...defaultTheme.fontFamily.sans],
      heading: ['Chillax', ...defaultTheme.fontFamily.sans],
      title: ['Clash Grotesk', ...defaultTheme.fontFamily.sans],
      minecraft: ['Minecraft Changed', ...defaultTheme.fontFamily.sans],
      mono: ['JetBrains Mono', ...defaultTheme.fontFamily.mono],
    },
    fontSize: {
      //! note: don't use fractional-px fonts
      caption: '0.625rem',
      small: '0.75rem',
      medium: '0.875rem',
      h3: '1rem',
      h2: '1.25rem',
      h1: '1.75rem',
      title: '2.625rem',
      '2xlarge': '2rem',
      '3xlarge': '2.5rem',
      '4xlarge': '3rem',
      '5xlarge': '3.5rem',
      '6xlarge': '4rem',
    },
    letterSpacing: {
      tight: '-0.04em',
      medium: '-0.01em',
      normal: '0',
      wide: '0.04em',
    },
    dropShadow: {
      sm: '0 0 transparent',
      md: '0 3px 6px var(--shadow-color)',
      lg: '0 8px 24px var(--shadow-color)',
      xl: '0 12px 48px var(--shadow-color)',
    },
    fontWeight: {
      medium: 450,
      mediumbold: 500,
      bold: 550,
      extrabold: 700,
    },
    extend: {
      transitionProperty: {
        //! animation stuff, added some extra transition animations (do we even ues these?)
        height: 'height',
        width: 'width',
        spacing: 'margin, padding',
        dimensions: 'height, width',
      },
    },
  },
  plugins: [
    function ({ addVariant }) {
      addVariant('child', '& > *');
    },
    require('@headlessui/tailwindcss')({ prefix: 'ui' }),
  ],
};
