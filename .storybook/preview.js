import 'tailwindcss/tailwind.css';

export const parameters = {
  actions: { argTypesRegex: "^on[A-Z].*" },
  controls: {
    matchers: {
      color: /(background|color)$/i,
      date: /Date$/,
    },
  },
  backgrounds: { default: 'gray-800', values: [{
    name: 'gray-800',
    value: '#26282C',
  }] },
}
