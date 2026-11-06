// prettier.config.js
/** @type {import('prettier').Config & import('prettier-plugin-tailwindcss').PluginOptions} */
module.exports = {
  semi: true,
  trailingComma: 'es5',
  singleQuote: true,
  tabWidth: 2,
  useTabs: false,
  plugins: ['prettier-plugin-tailwindcss'],
  // Tailwind CSS v4 entry point (theme and config are loaded from it)
  tailwindStylesheet: './src/globals.css',
};
