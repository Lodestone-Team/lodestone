import type { StorybookConfig } from '@storybook/nextjs-vite';

const config: StorybookConfig = {
  // No MDX: @storybook/addon-docs (formerly part of addon-essentials) is not installed
  stories: ['../src/**/*.stories.@(js|jsx|ts|tsx)'],
  /**
   * Expose the public folder to storybook as static, plus the MSW service
   * worker, which is only needed by Storybook and must not ship with the app
   */
  staticDirs: ['../public', './msw'],
  // Actions, controls, backgrounds, viewport and interactions are built into
  // Storybook 10. The Next.js framework mocks next/router itself, resolves the
  // tsconfig `paths` imports and runs the PostCSS (Tailwind) config.
  addons: [],
  framework: '@storybook/nextjs-vite',
};

export default config;
