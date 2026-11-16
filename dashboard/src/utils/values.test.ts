// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  gamePhrase,
  homeOf,
  instanceLoaderPhrase,
  loaderName,
  loaderPhrase,
  middleTruncate,
  productPhrase,
  shortenPath,
  splitShown,
} from './values.ts';

describe('product phrase', () => {
  test('a product and its version are one phrase', () => {
    assert.equal(productPhrase('Fabric', '0.16.9'), 'Fabric 0.16.9');
    assert.equal(gamePhrase('1.21.1'), 'Minecraft 1.21.1');
    assert.equal(loaderPhrase('neoforge', '21.1.5'), 'NeoForge 21.1.5');
  });
  test('a blank version leaves the product alone, and spaces are trimmed', () => {
    assert.equal(productPhrase('Fabric', null), 'Fabric');
    assert.equal(productPhrase('Fabric', ''), 'Fabric');
    assert.equal(productPhrase(' Fabric ', '  '), 'Fabric');
    assert.equal(productPhrase('Paper', ' 1.21.4 '), 'Paper 1.21.4');
    assert.equal(gamePhrase(undefined), 'Minecraft');
  });
  test('known loaders get their product name, unknown ones a capital', () => {
    assert.equal(loaderName('fabric'), 'Fabric');
    assert.equal(loaderName('bungeecord'), 'BungeeCord');
    assert.equal(loaderName('FORGE'), 'Forge');
    assert.equal(loaderName('vanilla'), 'Minecraft');
    assert.equal(loaderName(' liteloader '), 'Liteloader');
    assert.equal(loaderName(''), '');
  });
  test('an instance shows its loader, except vanilla and other games', () => {
    const java = (variant: object) =>
      ({ type: 'MinecraftJava', variant }) as Parameters<
        typeof instanceLoaderPhrase
      >[0];
    assert.equal(
      instanceLoaderPhrase(java({ type: 'Fabric' }), '0.16.9'),
      'Fabric 0.16.9'
    );
    assert.equal(instanceLoaderPhrase(java({ type: 'Paper' }), null), 'Paper');
    assert.equal(
      instanceLoaderPhrase(java({ type: 'Other', name: 'Purpur' }), '2200'),
      'Purpur 2200'
    );
    assert.equal(instanceLoaderPhrase(java({ type: 'Vanilla' }), null), null);
    assert.equal(
      instanceLoaderPhrase({ type: 'MinecraftBedrock' }, '1.0'),
      null
    );
  });
});

describe('middleTruncate', () => {
  test('text that fits is unchanged', () => {
    assert.equal(middleTruncate('sodium.jar', 20), 'sodium.jar');
    assert.equal(middleTruncate('', 5), '');
  });
  test('keeps the extension and fits the limit', () => {
    const name = 'fabric-api-0.116.17+1.21.1-very-long-build.jar';
    const out = middleTruncate(name, 24);
    assert.equal(Array.from(out).length, 24);
    assert.ok(out.endsWith('.jar'), out);
    assert.ok(out.startsWith('fabric-api'), out);
    assert.ok(out.includes('…'));
  });
  test('keeps at least a third at the end without an extension', () => {
    const out = middleTruncate('abcdefghijklmnopqrstuvwxyz', 13);
    assert.equal(out, 'abcdefgh…wxyz');
  });
  test('never splits an emoji', () => {
    const text = '😀'.repeat(30);
    const out = middleTruncate(text, 10);
    assert.equal(Array.from(out).length, 10);
    assert.ok(Array.from(out).every((c) => c === '😀' || c === '…'));
  });
  test('a tiny limit keeps the end', () => {
    assert.equal(middleTruncate('abcdefgh.jar', 4), '…jar');
    assert.equal(middleTruncate('abcdefgh', 1), '…');
  });
  test('keepTail keeps more of the end', () => {
    assert.ok(
      middleTruncate('mc1.21.1-0.6.0-fabric-build', 16, {
        keepTail: 10,
      }).endsWith('ric-build')
    );
  });
});

describe('shortenPath', () => {
  test('finds the home folder by the path shape', () => {
    assert.equal(homeOf('/Users/ana/.lodestone'), '/Users/ana');
    assert.equal(homeOf('/home/ana'), '/home/ana');
    assert.equal(homeOf('/root/x'), '/root');
    assert.equal(homeOf('C:\\Users\\Ana\\x'), 'C:\\Users\\Ana');
    assert.equal(homeOf('d:/users/ana/x'), 'd:/users/ana');
    assert.equal(homeOf('/rooted/x'), null);
    assert.equal(homeOf('/srv/lodestone'), null);
  });
  test('mac and linux homes become ~', () => {
    assert.equal(shortenPath('/Users/ana/.lodestone'), '~/.lodestone');
    assert.equal(shortenPath('/home/ana/servers/a'), '~/servers/a');
    assert.equal(shortenPath('/root'), '~');
  });
  test('windows paths keep their backslashes', () => {
    assert.equal(
      shortenPath('C:\\Users\\Ana\\.lodestone\\instances'),
      '~\\.lodestone\\instances'
    );
  });
  test('a path without a home stays, and a short one is untouched', () => {
    assert.equal(shortenPath('/srv/lodestone'), '/srv/lodestone');
    assert.equal(shortenPath('mods/'), 'mods/');
  });
  test('a long path keeps its start and last two parts', () => {
    assert.equal(
      shortenPath(
        '/Users/ana/.lodestone/instances/a-really-long-folder/more/instances/old-pack',
        { max: 30 }
      ),
      '~/…/instances/old-pack'
    );
    assert.equal(
      shortenPath('/srv/a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w', {
        max: 20,
      }),
      '/…/v/w'
    );
    assert.equal(
      shortenPath('/srv/a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/', {
        max: 20,
      }),
      '/…/v/w/'
    );
  });
  test('splitShown counts what is left', () => {
    assert.deepEqual(splitShown([1, 2, 3, 4, 5, 6, 7], 5), {
      shown: [1, 2, 3, 4, 5],
      rest: 2,
    });
    assert.deepEqual(splitShown([1], 5), { shown: [1], rest: 0 });
  });
});
