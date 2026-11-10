// Starts the Worker under `wrangler dev --local` (workerd, with the Durable Object and
// the rate limiters simulated) on a free port, with fresh state, for one test file.

import { before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const windows = process.platform === 'win32';

export function useServer(extraArgs = []) {
  const port = 18900 + Math.floor(Math.random() * 1000);
  // Its own debugger port too: two wrangler devs would otherwise both want 9229.
  const inspectorPort = port + 1000;
  const server = { base: `http://127.0.0.1:${port}`, ws: `ws://127.0.0.1:${port}` };
  // wrangler keeps local state (the rate limiters too) between runs: start from nothing.
  const state = mkdtempSync(join(tmpdir(), 'lodestone-connect-test-'));
  let dev;

  before(async () => {
    const args = ['dev', '--local', '--port', String(port), '--inspector-port', String(inspectorPort), '--ip', '127.0.0.1', '--persist-to', state, ...extraArgs];
    // wrangler's script run with this Node (node_modules/.bin/wrangler is a shell script,
    // which Windows can't spawn). In its own process group, so stopping it stops workerd
    // too; on Windows, taskkill stops the tree instead.
    dev = spawn(process.execPath, [join(import.meta.dirname, '..', 'node_modules', 'wrangler', 'bin', 'wrangler.js'), ...args], {
      stdio: ['ignore', 'pipe', 'pipe'],
      detached: !windows,
      windowsHide: true,
      env: { ...process.env, WRANGLER_SEND_METRICS: 'false' },
    });
    let output = '';
    const keep = (d) => {
      output += d;
      if (process.env.CONNECT_TEST_LOG) process.stderr.write(d);
    };
    dev.stdout.on('data', keep);
    dev.stderr.on('data', keep);
    for (let i = 0; i < 120; i++) {
      try {
        if ((await fetch(`${server.base}/v1/health`)).ok) return;
      } catch {}
      await sleep(500);
    }
    throw new Error(`wrangler dev didn't start:\n${output}`);
  });

  after(() => {
    try {
      if (windows) spawnSync('taskkill', ['/pid', String(dev.pid), '/t', '/f'], { stdio: 'ignore' });
      else process.kill(-dev.pid, 'SIGTERM');
    } catch {}
    try {
      rmSync(state, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
    } catch {}
  });

  server.post = (path, body, headers = {}) =>
    fetch(`${server.base}${path}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', ...headers },
      body: typeof body === 'string' ? body : JSON.stringify(body),
    });
  server.get = (path, secret) =>
    fetch(`${server.base}${path}`, {
      headers: { 'x-lodestone-client': '1', ...(secret ? { authorization: `Bearer ${secret}` } : {}) },
    });
  server.create = async (candidates = ['203.0.113.5:4000']) => {
    const response = await server.post('/v1/invites', { candidates });
    assert.equal(response.status, 201);
    return response.json();
  };
  server.join = (code, candidates = ['198.51.100.7:5000']) => server.post(`/v1/invites/${code}/joins`, { candidates });
  /// The host's WebSocket, with its messages queued for `next()`.
  server.connect = (code, secret, after = 0) => new Socket(`${server.ws}/v1/invites/${code}/joins?after=${after}`, secret);
  return server;
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export class Socket {
  constructor(url, secret) {
    this.messages = [];
    this.waiting = [];
    this.ws = new WebSocket(url, { headers: secret ? { authorization: `Bearer ${secret}` } : {} });
    this.closed = new Promise((resolve) => {
      this.ws.addEventListener('close', (event) => resolve({ code: event.code, reason: event.reason }));
    });
    this.opened = new Promise((resolve, reject) => {
      this.ws.addEventListener('open', resolve);
      this.ws.addEventListener('error', reject);
    });
    this.ws.addEventListener('message', (event) => {
      const waiter = this.waiting.shift();
      if (waiter) waiter(event.data);
      else this.messages.push(event.data);
    });
  }

  /// The next message, failing after `ms` (and then a later message waits for the next
  /// call, rather than going to the one that gave up).
  next(ms = 5000) {
    if (this.messages.length) return Promise.resolve(this.messages.shift());
    return new Promise((resolve, reject) => {
      const waiter = (data) => {
        clearTimeout(timer);
        resolve(data);
      };
      const timer = setTimeout(() => {
        this.waiting = this.waiting.filter((w) => w !== waiter);
        reject(new Error('no message'));
      }, ms);
      this.waiting.push(waiter);
    });
  }

  async nextJson(ms) {
    return JSON.parse(await this.next(ms));
  }

  close() {
    this.ws.close();
  }
}
