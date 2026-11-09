// Builds www.lodestone.cc into website/dist:
//
//   /       the Lodestone Launcher landing page (website/src, copied as is)
//   /core   the Lodestone Core web dashboard (dashboard/, built with basePath /core)
//
// Netlify runs this (see netlify.toml). Locally:
//
//   node website/build.mjs                  # landing page and dashboard
//   node website/build.mjs --landing-only   # landing page only, no dashboard build
//   node website/serve.mjs                  # preview dist with Netlify's redirects
import { execSync } from 'node:child_process';
import { cpSync, existsSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteDir = dirname(fileURLToPath(import.meta.url));
const dashboardDir = join(websiteDir, '..', 'dashboard');
const dist = join(websiteDir, 'dist');
const landingOnly = process.argv.includes('--landing-only');

export const DASHBOARD_BASE_PATH = '/core';

function run(command, cwd, env = {}) {
  console.log(`\n$ ${command}  (in ${cwd})`);
  execSync(command, { cwd, stdio: 'inherit', env: { ...process.env, ...env } });
}

rmSync(dist, { recursive: true, force: true });
cpSync(join(websiteDir, 'src'), dist, { recursive: true });
console.log(`Copied the landing page to ${dist}`);

if (!landingOnly) {
  // Netlify starts from a clean checkout; locally, reuse installed modules.
  if (process.env.NETLIFY || !existsSync(join(dashboardDir, 'node_modules'))) {
    run('npm ci', dashboardDir);
  }
  run('npm run build', dashboardDir, { DASHBOARD_BASE_PATH });
  cpSync(join(dashboardDir, 'out'), join(dist, DASHBOARD_BASE_PATH.slice(1)), {
    recursive: true,
  });
  console.log(`Copied the dashboard to ${join(dist, DASHBOARD_BASE_PATH.slice(1))}`);
}
