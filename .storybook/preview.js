import 'tailwindcss/tailwind.css';

// Registers the msw addon
import { initialize, mswDecorator } from 'msw-storybook-addon';

// Initialize MSW
initialize();

// Provide the MSW addon decorator globally
export const decorators = [mswDecorator];

export const parameters = {
  actions: { argTypesRegex: "^on[A-Z].*" },
  controls: {
    matchers: {
      color: /(background|color)$/i,
      date: /Date$/,
    },
  },
  backgrounds: {
    default: 'gray-800',
    values: [{
      name: 'gray-800',
      value: '#26282C',
    },
    {
      name: 'gray-700',
      value: '#36393F',
    }]
  },
}
