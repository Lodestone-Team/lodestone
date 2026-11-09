// Serves website/dist on http://localhost:8888 with the same redirects as
// netlify.toml, to check a build before it is deployed. Not used by Netlify.
import { createReadStream, existsSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const dist = join(dirname(fileURLToPath(import.meta.url)), 'dist');
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

// [pattern, target, status], in netlify.toml's order
const REDIRECTS = [
  [/^\/first_setup$/, () => '/core/first_setup', 301],
  [/^\/login\/(.*)$/, (m) => `/core/login/${m[1]}`, 301],
  [/^\/dashboard\/(.*)$/, (m) => `/core/dashboard/${m[1]}`, 301],
  [/^\/settings\/(.*)$/, (m) => `/core/settings/${m[1]}`, 301],
];

function file(pathname) {
  const path = normalize(join(dist, decodeURIComponent(pathname)));
  if (!path.startsWith(dist)) return null;
  if (existsSync(path) && statSync(path).isFile()) return path;
  if (existsSync(join(path, 'index.html'))) return join(path, 'index.html');
  if (existsSync(`${path}.html`)) return `${path}.html`;
  return null;
}

createServer((req, res) => {
  const url = new URL(req.url, `http://localhost:${port}`);
  let path = file(url.pathname);
  if (!path) {
    for (const [pattern, target, status] of REDIRECTS) {
      const m = url.pathname.match(pattern);
      if (m) {
        res.writeHead(status, { Location: target(m) + url.search });
        return res.end();
      }
    }
    if (url.pathname === '/core' || url.pathname.startsWith('/core/')) {
      path = join(dist, 'core', 'index.html');
    }
  }
  if (!path) {
    res.writeHead(404, { 'Content-Type': 'text/plain' });
    return res.end('Not found');
  }
  res.writeHead(200, { 'Content-Type': TYPES[extname(path)] || 'application/octet-stream' });
  createReadStream(path).pipe(res);
}).listen(port, () => console.log(`Serving ${dist} on http://localhost:${port}`));
