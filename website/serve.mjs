// Serves website/dist on http://localhost:8888 the way Cloudflare does: the
// rules in src/_redirects first, then files, then worker.js's fallback for the
// dashboard's routes. For a quick look at a build; `npx wrangler dev` (run in
// website/) runs the real thing.
import { createReadStream, existsSync, readFileSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteDir = dirname(fileURLToPath(import.meta.url));
const dist = join(websiteDir, 'dist');
const port = Number(process.env.PORT) || 8888;

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.txt': 'text/plain',
};

// "/login/*  /core/login/:splat  301" -> { pattern, target, status }
const redirects = readFileSync(join(websiteDir, 'src', '_redirects'), 'utf8')
  .split('\n')
  .map((line) => line.trim())
  .filter((line) => line && !line.startsWith('#'))
  .map((line) => {
    const [from, to, status = '302'] = line.split(/\s+/);
    const escaped = from.replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace('*', '(.*)');
    return { pattern: new RegExp(`^${escaped}$`), to, status: Number(status) };
  });

function file(pathname) {
  const path = normalize(join(dist, decodeURIComponent(pathname)));
  if (!path.startsWith(dist)) return null;
  if (existsSync(path) && statSync(path).isFile()) return path;
  if (existsSync(join(path, 'index.html'))) return join(path, 'index.html');
  if (existsSync(`${path}.html`)) return `${path}.html`;
  return null;
}

function send(res, status, path) {
  res.writeHead(status, { 'Content-Type': TYPES[extname(path)] || 'application/octet-stream' });
  createReadStream(path).pipe(res);
}

createServer((req, res) => {
  const url = new URL(req.url, `http://localhost:${port}`);

  for (const { pattern, to, status } of redirects) {
    const m = url.pathname.match(pattern);
    if (m) {
      res.writeHead(status, { Location: to.replace(':splat', m[1] ?? '') + url.search });
      return res.end();
    }
  }

  const path = file(url.pathname);
  if (path) return send(res, 200, path);

  // worker.js
  const lastSegment = url.pathname.slice(url.pathname.lastIndexOf('/') + 1);
  const inDashboard = url.pathname === '/core' || url.pathname.startsWith('/core/');
  if (inDashboard && !lastSegment.includes('.')) {
    return send(res, 200, join(dist, 'core', 'index.html'));
  }

  res.writeHead(404, { 'Content-Type': 'text/plain' });
  res.end('Not found');
}).listen(port, () => console.log(`Serving ${dist} on http://localhost:${port}`));
