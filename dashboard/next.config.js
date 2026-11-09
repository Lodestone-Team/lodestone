const { PHASE_DEVELOPMENT_SERVER } = require('next/constants');

/** @type {(phase: string) => import('next').NextConfig} */
module.exports = (phase) => {
  // The path to serve the dashboard under, e.g. "/host/app" for lodestone.cc (see
  // website/build.mjs). Empty for Tauri and everything else.
  const basePath = process.env.DASHBOARD_BASE_PATH || '';

  /** @type {import('next').NextConfig} */
  const nextConfig = {
    // Static HTML export to out/ (replaces the removed `next export` command)
    output: 'export',
    ...(basePath ? { basePath } : {}),
    // Inlined into the bundle (see src/utils/version.ts), so package.json is
    // not bundled just to read the version.
    env: {
      NEXT_PUBLIC_DASHBOARD_VERSION: require('./package.json').version,
      // Read by src/utils/basePath.ts
      NEXT_PUBLIC_BASE_PATH: basePath,
    },
  };

  // The dashboard is a react-router SPA served from a single page. Rewrites are
  // not applied to a static export, so only add the catch-all for `next dev`.
  if (phase === PHASE_DEVELOPMENT_SERVER) {
    nextConfig.rewrites = async () => [
      {
        source: '/:any*',
        destination: '/',
      },
    ];
  }

  return nextConfig;
};
