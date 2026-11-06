import type { Preview } from '@storybook/nextjs-vite';
// Starts the MSW worker (served from .storybook/msw) and applies the
// `parameters.msw` handlers of each story
import { mswLoader } from 'msw-storybook-addon/csf3';
// Tailwind CSS v4 entry, which also loads the Tailwind base styles
import '../src/globals.css';

const preview: Preview = {
  loaders: [mswLoader()],
  parameters: {
    layout: 'centered',
    actions: { argTypesRegex: '^on[A-Z].*' },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/,
      },
    },
    backgrounds: {
      options: {
        'gray-700': { name: 'gray-700', value: '#36393F' },
        'gray-750': { name: 'gray-750', value: '#303338' },
        'gray-800': { name: 'gray-800', value: '#2B2D32' },
        'gray-850': { name: 'gray-850', value: '#26282C' },
        'gray-875': { name: 'gray-875', value: '#212327' },
        'gray-900': { name: 'gray-900', value: '#1D1E21' },
      },
    },
  },
  initialGlobals: {
    backgrounds: { value: 'gray-800' },
  },
};

export default preview;
