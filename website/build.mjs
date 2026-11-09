// Builds www.lodestone.cc into website/dist:
//
//   /       the Lodestone Launcher landing page (website/src, copied as is)
//   /core   the Lodestone Core web dashboard (dashboard/, built with basePath /core)
//
// Cloudflare's build runs this from website/, then deploys dist with
// wrangler.jsonc. Locally:
//
//   node website/build.mjs                  # landing page and dashboard
//   node website/build.mjs --landing-only   # landing page only, no dashboard build
//   node website/serve.mjs                  # preview dist with the same redirects
import { execSync } from 'node:child_process';
import { cpSync, existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { coreDownloads, formatDownloads } from './stats.mjs';

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

// Lodestone Core's download count. Sources that can't be read fall back to
// their last known counts (see stats.mjs), so a GitHub hiccup never fails the
// deploy or lowers the number.
const downloads = await coreDownloads();
if (downloads !== null) {
  const page = join(dist, 'index.html');
  const html = readFileSync(page, 'utf8');
  const marker = /(<span data-core-downloads>)[^<]*(<\/span>)/;
  if (!marker.test(html)) throw new Error('index.html has no data-core-downloads span');
  writeFileSync(page, html.replace(marker, `$1${formatDownloads(downloads)}$2`));
}

if (!landingOnly) {
  // CI starts from a clean checkout (Cloudflare's build sets CI); locally,
  // reuse installed modules.
  if (process.env.CI || !existsSync(join(dashboardDir, 'node_modules'))) {
    run('npm ci', dashboardDir);
  }
  run('npm run build', dashboardDir, { DASHBOARD_BASE_PATH });
  cpSync(join(dashboardDir, 'out'), join(dist, DASHBOARD_BASE_PATH.slice(1)), {
    recursive: true,
  });
  console.log(`Copied the dashboard to ${join(dist, DASHBOARD_BASE_PATH.slice(1))}`);
}
