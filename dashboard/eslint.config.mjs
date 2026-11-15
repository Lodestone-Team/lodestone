import { defineConfig, globalIgnores } from 'eslint/config';
import nextVitals from 'eslint-config-next/core-web-vitals';
import nextTs from 'eslint-config-next/typescript';
import prettier from 'eslint-config-prettier/flat';
import storybook from 'eslint-plugin-storybook';
import tailwind from 'eslint-plugin-tailwindcss';

export default defineConfig([
  globalIgnores([
    '.next/**',
    'out/**',
    'build/**',
    'storybook-static/**',
    'src-tauri/**',
    '.storybook/msw/**',
    'public/monaco/**',
    'next-env.d.ts',
  ]),
  ...nextVitals,
  ...nextTs,
  tailwind.configs.recommended,
  ...storybook.configs['flat/recommended'],
  {
    settings: {
      tailwindcss: {
        cssConfigPath: './src/globals.css',
        functions: ['cn', 'clsx', 'variants'],
      },
    },
    rules: {
      'no-restricted-imports': [
        'error',
        {
          name: 'react-router',
          importNames: ['useNavigate'],
          message: 'Please use functions from BrowserLocationContext instead',
        },
        {
          name: '@tauri-apps/api',
          message:
            'Please use the global tauri object from tauriUtil.ts instead',
        },
        {
          name: 'next/router',
          message: 'Please use react-router instead',
        },
        {
          name: 'tailwind-merge',
          message: 'Please use "myTwMerge" from "utils.ts" instead',
        },
      ],
      // The global `location` includes the base path (/host/app on the hosted
      // dashboard), which react-router's paths don't. Read the router's
      // location, or stripBasePath(window.location.pathname).
      'no-restricted-globals': [
        'error',
        {
          name: 'location',
          message:
            'Use the location from react-router or BrowserLocationContext, or stripBasePath(window.location.pathname)',
        },
      ],
      '@next/next/no-img-element': 'off',
      // New React Compiler rules in eslint-plugin-react-hooks 7 (errors in its
      // recommended preset). Reported as warnings until the code is migrated.
      'react-hooks/config': 'warn',
      'react-hooks/error-boundaries': 'warn',
      'react-hooks/gating': 'warn',
      'react-hooks/globals': 'warn',
      'react-hooks/immutability': 'warn',
      'react-hooks/preserve-manual-memoization': 'warn',
      'react-hooks/purity': 'warn',
      'react-hooks/refs': 'warn',
      'react-hooks/set-state-in-effect': 'warn',
      'react-hooks/set-state-in-render': 'warn',
      'react-hooks/static-components': 'warn',
      'react-hooks/use-memo': 'warn',
      'no-unused-vars': 'off',
      // 'warn' in typescript-eslint 5's recommended preset, 'error' since 6
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/no-unused-vars': [
        'warn',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],
    },
  },
  {
    // CommonJS config files
    files: ['*.config.js'],
    languageOptions: { sourceType: 'commonjs' },
    rules: {
      '@typescript-eslint/no-require-imports': 'off',
    },
  },
  prettier,
]);
