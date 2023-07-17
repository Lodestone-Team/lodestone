import 'tailwindcss/tailwind.css';
import '../src/globals.css';

// Registers the msw addon
import { initialize, mswDecorator } from 'msw-storybook-addon';
import { RouterContext } from 'next/dist/shared/lib/router-context';

// Initialize MSW
initialize();

// Provide the MSW addon decorator globally
export const decorators = [mswDecorator];

export const parameters = {
  layout: 'centered',
  actions: { argTypesRegex: '^on[A-Z].*' },
  controls: {
    matchers: {
      color: /(background|color)$/i,
      date: /Date$/,
    },
  },
  backgrounds: {
    default: 'gray-800',
    values: [
      {
        name: 'gray-700',
        value: '#36393F',
      },
      {
        name: 'gray-750',
        value: '#303338',
      },
      {
        name: 'gray-800',
        value: '#2B2D32',
      },
      {
        name: 'gray-850',
        value: '#26282C',
      },
      {
        name: 'gray-875',
        value: '#212327',
      },
      {
        name: 'gray-900',
        value: '#1D1E21',
      },
    ],
  },
  nextRouter: {
    Provider: RouterContext.Provider,
  },
};
