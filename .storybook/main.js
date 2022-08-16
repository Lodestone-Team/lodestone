module.exports = {
  "stories": [
    "../src/**/*.stories.mdx",
    "../src/**/*.stories.@(js|jsx|ts|tsx)"
  ],
  /** Expose public folder to storybook as static */
  "staticDirs": ['../public'],
  "addons": [
    '@storybook/addon-links',
    '@storybook/addon-essentials',
    '@storybook/addon-interactions',
    '@storybook/addon-postcss',
  ],
  "framework": '@storybook/react',
  "core": {
    "builder": '@storybook/builder-webpack5',
  },
};
