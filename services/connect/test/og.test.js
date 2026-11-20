// The link pages' OpenGraph tags and cards (what Discord and other apps show when an
// invite or friend link is pasted). Against `wrangler dev --local`. Run with `npm test`.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer } from './server.js';

const s = useServer();

/// The page's `<meta>` tags as a map from property/name to content.
function metas(html) {
  const found = {};
  for (const tag of html.matchAll(/<meta\s+(property|name)="([^"]+)"\s+content="([^"]*)">/g)) found[tag[2]] = tag[3];
  return found;
}

const ORIGIN = 'https://connect.lodestone.cc';

for (const [path, image, title] of [
  ['j', 'invite', 'Join my Minecraft server'],
  ['f', 'friend', "Let's be friends on Lodestone"],
]) {
  test(`the ${image} page has a large image card`, async () => {
    const html = await (await fetch(`${s.base}/${path}/ABCDEFGH23`)).text();
    const tags = metas(html);
    assert.equal(tags['og:title'], title);
    assert.ok(tags['og:description']);
    assert.equal(tags['og:site_name'], 'Lodestone');
    assert.equal(tags['og:type'], 'website');
    assert.equal(tags['og:image'], `${ORIGIN}/og/${image}.png`);
    assert.equal(tags['og:image:type'], 'image/png');
    assert.equal(tags['og:image:width'], '1200');
    assert.equal(tags['og:image:height'], '630');
    assert.ok(tags['og:image:alt']);
    assert.match(tags['theme-color'], /^#[0-9a-f]{6}$/);
    assert.equal(tags['twitter:card'], 'summary_large_image');
    assert.equal(tags['twitter:image'], tags['og:image']);

    // The card is the same for every code: the page names no server or friend.
    const other = await (await fetch(`${s.base}/${path}/23456789AB`)).text();
    assert.equal(other, html);
    assert.doesNotMatch(html, /ABCDEFGH23/);

    // Discord's crawler fetches the card, not the page, so the policy still allows no images.
    const csp = (await fetch(`${s.base}/${path}/ABCDEFGH23`)).headers.get('content-security-policy');
    assert.match(csp, /default-src 'none'/);
    assert.doesNotMatch(csp, /img-src/);
  });

  test(`the ${image} card is a 1200x630 PNG that caches for long`, async () => {
    const response = await fetch(`${s.base}/og/${image}.png`);
    assert.equal(response.status, 200);
    assert.equal(response.headers.get('content-type'), 'image/png');
    assert.match(response.headers.get('cache-control'), /public, max-age=\d{6,}/);
    assert.equal(response.headers.get('x-content-type-options'), 'nosniff');
    const bytes = Buffer.from(await response.arrayBuffer());
    assert.deepEqual([...bytes.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], 'a PNG');
    // The IHDR chunk comes first: width, then height.
    assert.equal(bytes.readUInt32BE(16), 1200);
    assert.equal(bytes.readUInt32BE(20), 630);
    assert.ok(bytes.length < 500_000, 'small enough for Discord to fetch quickly');

    const head = await fetch(`${s.base}/og/${image}.png`, { method: 'HEAD' });
    assert.equal(head.status, 200);
    assert.equal(head.headers.get('content-type'), 'image/png');
  });
}

/// The page's Discord component embed: the JSON in `<script id="discord:component-embed">`
/// (in <head>), its size in bytes as served, and the raw text.
function componentEmbedOf(html) {
  const match = /<script id="discord:component-embed" type="application\/json">(.*?)<\/script>/s.exec(html);
  assert.ok(match, 'the page has a component embed');
  assert.ok(match.index < html.indexOf('</head>'), 'in <head>');
  return { embed: JSON.parse(match[1]), bytes: Buffer.byteLength(match[1]), raw: match[1] };
}

/// Every component in the tree, the container first.
function allComponents(component) {
  return [component, ...(component.components || []).flatMap(allComponents), ...(component.accessory ? [component.accessory] : [])];
}

for (const [path, image, title] of [
  ['j', 'invite', 'Join my Minecraft server'],
  ['f', 'friend', "Let's be friends on Lodestone"],
]) {
  test(`the ${image} page has a Discord component embed with the card and link buttons`, async () => {
    const html = await (await fetch(`${s.base}/${path}/ABCDEFGH23`)).text();
    const { embed, bytes, raw } = componentEmbedOf(html);

    // Discord's rules (developers/link-previews/component-embeds): one Container at the
    // top, at most 40 components, 3,000 bytes, only these types, link buttons only.
    assert.deepEqual(Object.keys(embed), ['component']);
    assert.equal(embed.component.type, 17);
    assert.equal(embed.component.accent_color, parseInt(metas(html)['theme-color'].slice(1), 16));
    assert.ok(bytes <= 3000, `${bytes} bytes`);
    assert.doesNotMatch(raw, /[<>&]/, 'nothing in it can end the script tag');
    const components = allComponents(embed.component);
    assert.ok(components.length <= 40);
    for (const c of components) assert.ok([1, 2, 9, 10, 11, 12, 14, 17].includes(c.type), `type ${c.type}`);
    assert.equal(components.filter((c) => c.type === 17).length, 1);

    const gallery = components.find((c) => c.type === 12);
    assert.deepEqual(gallery.items.map((i) => i.media.url), [`${ORIGIN}/og/${image}.png`]);
    assert.ok(gallery.items[0].description);

    const text = components.filter((c) => c.type === 10).map((c) => c.content).join('\n');
    assert.match(text, new RegExp(`^### ${title}$`, 'm'));
    assert.match(text, /Lodestone Launcher/);

    const buttons = components.filter((c) => c.type === 2);
    assert.deepEqual(buttons.map((b) => b.url), ['https://www.lodestone.cc/launcher/', 'https://www.lodestone.cc/']);
    for (const b of buttons) {
      assert.equal(b.style, 5, 'link buttons only');
      assert.ok(b.label && b.label.length <= 80);
      assert.deepEqual(Object.keys(b).sort(), ['label', 'style', 'type', 'url'], 'any other key invalidates the embed');
    }

    // Like the card, the same for every code: it names no server, friend or code.
    assert.doesNotMatch(raw, /ABCDEFGH23/);
  });
}

test('the two cards differ, and only they exist', async () => {
  const invite = Buffer.from(await (await fetch(`${s.base}/og/invite.png`)).arrayBuffer());
  const friend = Buffer.from(await (await fetch(`${s.base}/og/friend.png`)).arrayBuffer());
  assert.notDeepEqual(invite, friend);
  for (const path of ['/og/other.png', '/og/invite', '/og/', '/og/invite.png/x', '/og/__proto__.png', '/og/constructor.png']) {
    assert.equal((await fetch(`${s.base}${path}`)).status, 404, path);
  }
  assert.equal((await fetch(`${s.base}/og/invite.png`, { method: 'POST' })).status, 404);
});
