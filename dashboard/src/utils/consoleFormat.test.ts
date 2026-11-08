// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  BOLD,
  DIM,
  INVERSE,
  ITALIC,
  MIN_CONTRAST,
  RGB_BASE,
  STRIKE,
  UNDERLINE,
  contrast,
  detectLevel,
  parseConsoleLine,
  readableColor,
  xterm256,
} from './consoleFormat.ts';

const E = '\x1b';

// [text, fg, bg, flags] per segment, for short assertions
const segs = (message: string) =>
  parseConsoleLine(message).segments.map((s) => [s.text, s.fg, s.bg, s.flags]);

describe('SGR', () => {
  test('plain text is one unstyled segment', () => {
    const line = parseConsoleLine('hello world');
    assert.deepEqual(segs('hello world'), [['hello world', null, null, 0]]);
    assert.equal(line.styled, false);
  });

  test('16 colours, bright colours and backgrounds', () => {
    assert.deepEqual(segs(`${E}[31mred${E}[0m ${E}[92mgreen${E}[44mon blue`), [
      ['red', 1, null, 0],
      [' ', null, null, 0],
      ['green', 10, null, 0],
      ['on blue', 10, 4, 0],
    ]);
    assert.deepEqual(segs(`${E}[103mx`), [['x', null, 11, 0]]);
  });

  test('combined parameters in one sequence', () => {
    assert.deepEqual(segs(`${E}[1;3;4;9;2;7;33mx`), [
      ['x', 3, null, BOLD | ITALIC | UNDERLINE | STRIKE | DIM | INVERSE],
    ]);
  });

  test('turning single attributes off', () => {
    assert.deepEqual(
      segs(`${E}[1;3;4mA${E}[22mB${E}[23mC${E}[24mD${E}[1;31mE${E}[39mF`),
      [
        ['A', null, null, BOLD | ITALIC | UNDERLINE],
        ['B', null, null, ITALIC | UNDERLINE],
        ['C', null, null, UNDERLINE],
        ['D', null, null, 0],
        ['E', 1, null, BOLD],
        ['F', null, null, BOLD],
      ]
    );
  });

  test('reset: 0, empty parameters and a bare m', () => {
    assert.deepEqual(segs(`${E}[1;31ma${E}[mb${E}[1;31mc${E}[0md`), [
      ['a', 1, null, BOLD],
      ['b', null, null, 0],
      ['c', 1, null, BOLD],
      ['d', null, null, 0],
    ]);
    // a leading empty parameter is a 0
    assert.deepEqual(segs(`${E}[31m${E}[;1mx`), [['x', null, null, BOLD]]);
  });

  test('256 colours', () => {
    assert.deepEqual(segs(`${E}[38;5;208mx${E}[48;5;21my`), [
      ['x', 208, null, 0],
      ['y', 208, 21, 0],
    ]);
    assert.deepEqual(segs(`${E}[38:5:9mx`), [['x', 9, null, 0]]);
    assert.equal(xterm256(16), 0x000000);
    assert.equal(xterm256(196), 0xff0000);
    assert.equal(xterm256(231), 0xffffff);
    assert.equal(xterm256(232), 0x080808);
    assert.equal(xterm256(255), 0xeeeeee);
  });

  test('truecolor, with semicolons and colons', () => {
    const orange = RGB_BASE + 0xff8000;
    assert.deepEqual(segs(`${E}[38;2;255;128;0mx`), [['x', orange, null, 0]]);
    assert.deepEqual(segs(`${E}[38:2:255:128:0mx`), [['x', orange, null, 0]]);
    // with the (empty) colour space id
    assert.deepEqual(segs(`${E}[38:2::255:128:0mx`), [['x', orange, null, 0]]);
    assert.deepEqual(segs(`${E}[1;48;2;0;0;255;4mx`), [
      ['x', null, RGB_BASE + 0xff, BOLD | UNDERLINE],
    ]);
  });

  test('parameters after an extended colour still apply', () => {
    assert.deepEqual(segs(`${E}[38;5;100;1mx`), [['x', 100, null, BOLD]]);
  });

  test('a style lasts across chunks of the line until reset', () => {
    assert.deepEqual(segs(`${E}[32m[12:00] ${E}[1mDone${E}[22m (1s)`), [
      ['[12:00] ', 2, null, 0],
      ['Done', 2, null, BOLD],
      [' (1s)', 2, null, 0],
    ]);
  });

  test('the style starts plain again after a newline in the event', () => {
    assert.deepEqual(segs(`${E}[31mone\ntwo`), [
      ['one', 1, null, 0],
      ['\ntwo', null, null, 0],
    ]);
  });

  test('redundant sequences merge into one segment', () => {
    assert.deepEqual(segs(`${E}[31ma${E}[31mb${E}[0m${E}[0mc`), [
      ['ab', 1, null, 0],
      ['c', null, null, 0],
    ]);
  });
});

describe('other escapes are removed', () => {
  test('cursor movement, erase and private modes', () => {
    const line = parseConsoleLine(
      `${E}[2K${E}[1G${E}[?25l>${E}[3A ok${E}[?2004h${E}[>4;1m`
    );
    assert.equal(line.plain, '> ok');
    assert.equal(line.styled, false);
  });

  test('OSC titles and hyperlinks, ended by BEL or ST', () => {
    assert.equal(
      parseConsoleLine(
        `${E}]0;My title\x07before ${E}]8;;https://x.y${E}\\link${E}]8;;${E}\\ after`
      ).plain,
      'before link after'
    );
  });

  test('short escapes and charset selection', () => {
    assert.equal(
      parseConsoleLine(`${E}7a${E}8b${E}(Bc${E}=d${E}Me`).plain,
      'abcde'
    );
  });

  test('DCS, APC and C1 controls', () => {
    assert.equal(
      parseConsoleLine(`a${E}Pq#0;1${E}\\b${E}_x${E}\\c`).plain,
      'abc'
    );
    assert.equal(parseConsoleLine('a\x9b31mb\x9d0;t\x07c').plain, 'abc');
    assert.deepEqual(segs('\x9b31mx'), [['x', 1, null, 0]]);
  });

  test('control characters are dropped, tabs are kept', () => {
    assert.equal(
      parseConsoleLine('a\x07b\x08c\x00d\re\tf\x7f').plain,
      'abcde\tf'
    );
  });

  test('the trailing line break is dropped', () => {
    assert.equal(parseConsoleLine('done\r\n').plain, 'done');
    assert.equal(parseConsoleLine('a\nb\n').plain, 'a\nb');
  });
});

describe('malformed sequences', () => {
  test('a lone ESC at the end, or before a newline', () => {
    assert.equal(parseConsoleLine(`abc${E}`).plain, 'abc');
    assert.equal(parseConsoleLine(`a${E}\nb`).plain, 'a\nb');
  });

  test('a CSI cut off at the end of the line', () => {
    assert.equal(parseConsoleLine(`abc${E}[31;1`).plain, 'abc');
    assert.equal(parseConsoleLine(`abc${E}[`).plain, 'abc');
  });

  test('a CSI interrupted by a character that cannot be part of it', () => {
    // the § is read again as text (it isn't followed by a code)
    assert.equal(parseConsoleLine(`a${E}[31§ b`).plain, 'a§ b');
    assert.equal(parseConsoleLine(`a${E}[31\nb`).plain, 'a\nb');
  });

  test('an OSC that is never terminated ends at the newline', () => {
    assert.equal(parseConsoleLine(`a${E}]0;title\nb`).plain, 'a\nb');
  });

  test('invalid colours are ignored', () => {
    assert.deepEqual(segs(`${E}[38;5;300mx`), [['x', null, null, 0]]);
    assert.deepEqual(segs(`${E}[38;2;255;0mx`), [['x', null, null, 0]]);
    assert.deepEqual(segs(`${E}[38;2;256;0;0mx`), [['x', null, null, 0]]);
    assert.deepEqual(segs(`${E}[38;7;1mx`), [['x', null, null, 0]]);
  });

  test('unknown SGR codes are ignored', () => {
    assert.deepEqual(segs(`${E}[5;53;31;73mx`), [['x', 1, null, 0]]);
  });
});

describe('Minecraft § codes', () => {
  test('colours', () => {
    assert.deepEqual(segs('§aGreen §cRed §6Gold §7Gray'), [
      ['Green ', 10, null, 0],
      ['Red ', 9, null, 0],
      ['Gold ', 3, null, 0],
      ['Gray', 7, null, 0],
    ]);
  });

  test('formats, §r and a colour ending the formats', () => {
    assert.deepEqual(segs('§l§nBold§r plain §o§mx§eyellow'), [
      ['Bold', null, null, BOLD | UNDERLINE],
      [' plain ', null, null, 0],
      ['x', null, null, ITALIC | STRIKE],
      ['yellow', 11, null, 0],
    ]);
    assert.deepEqual(segs('§c§lBOLD RED§r.'), [
      ['BOLD RED', 9, null, BOLD],
      ['.', null, null, 0],
    ]);
  });

  test('§k is shown as plain text', () => {
    assert.deepEqual(segs('§kmagic'), [['magic', null, null, 0]]);
  });

  test('uppercase codes', () => {
    assert.deepEqual(segs('§Ax§Ly'), [
      ['x', 10, null, 0],
      ['y', 10, null, BOLD],
    ]);
  });

  test('hex colours', () => {
    assert.deepEqual(segs('§x§f§f§8§0§0§0orange'), [
      ['orange', RGB_BASE + 0xff8000, null, 0],
    ]);
    // incomplete: kept as text
    assert.equal(parseConsoleLine('§x§f§fab').plain, '§xab');
  });

  test('a § that is not a code stays', () => {
    assert.equal(parseConsoleLine('§ 5 §z §').plain, '§ 5 §z §');
    assert.equal(parseConsoleLine('Section §12').plain, 'Section 2');
  });

  test('& is never read as a code', () => {
    assert.deepEqual(segs('Tom &amp; Jerry &a &l'), [
      ['Tom &amp; Jerry &a &l', null, null, 0],
    ]);
  });

  test('mixed with ANSI', () => {
    assert.deepEqual(segs(`§a${E}[1mx${E}[0my`), [
      ['x', 10, null, BOLD],
      ['y', null, null, 0],
    ]);
  });
});

describe('log levels', () => {
  test('common server formats', () => {
    assert.equal(
      detectLevel("[12:00:00] [Server thread/WARN]: Can't keep up!"),
      'warn'
    );
    assert.equal(detectLevel('[12:00:00 WARN]: Plugin x is outdated'), 'warn');
    assert.equal(detectLevel('[12:00:00 ERROR]: Could not load'), 'error');
    assert.equal(
      detectLevel(
        '[05Nov2024 12:00:00.123] [main/FATAL] [net.minecraftforge/]: boom'
      ),
      'error'
    );
    assert.equal(detectLevel('[12:00:00] [Server thread/ERROR]: x'), 'error');
    assert.equal(detectLevel('[12:00:00 SEVERE]: x'), 'error');
    assert.equal(detectLevel('[WARN] something'), 'warn');
    assert.equal(detectLevel('[2024-01-01 12:00:00:000 WARN] Bedrock'), 'warn');
    assert.equal(detectLevel('ERROR: container failed'), 'error');
    assert.equal(detectLevel('WARNING: low memory'), 'warn');
  });

  test('INFO and unknown lines have no level', () => {
    assert.equal(
      detectLevel('[12:00:00] [Server thread/INFO]: Done (3.2s)!'),
      null
    );
    assert.equal(
      detectLevel('[12:00:00 INFO]: [MyPlugin] [ERROR] in the message'),
      null
    );
    assert.equal(detectLevel('<Steve> the server has an ERROR: lol'), null);
    assert.equal(detectLevel('Loading libraries, please wait...'), null);
    assert.equal(
      detectLevel('[12:00:00] [Server thread/INFO]: [WARN] x'),
      null
    );
  });

  test('only lines without styles get a level', () => {
    assert.equal(parseConsoleLine('[12:00:00 WARN]: x\n').level, 'warn');
    assert.equal(
      parseConsoleLine(`${E}[33m[12:00:00 WARN]: x${E}[m`).level,
      null
    );
    // resets alone don't count as styles
    assert.equal(parseConsoleLine(`[12:00:00 ERROR]: x${E}[m`).level, 'error');
  });
});

describe('plain text', () => {
  test('has no codes and matches the segments', () => {
    const message = `${E}[1;38;2;1;2;3m§aHi${E}[0m ${E}]0;t\x07there§r!\r\n`;
    const line = parseConsoleLine(message);
    assert.equal(line.plain, 'Hi there!');
    assert.equal(line.segments.map((s) => s.text).join(''), line.plain);
  });
});

describe('readable colours', () => {
  test('dark colours are lightened on the dark console', () => {
    const navy = 0x000080;
    const adjusted = readableColor(navy, 'dark');
    assert.notEqual(adjusted, navy);
    assert.ok(contrast(adjusted, 0x2b2d32) >= MIN_CONTRAST);
  });

  test('light colours are darkened on the light console', () => {
    for (const color of [0xffff00, 0xffffff, 0x55ffff, 0xffaa00]) {
      assert.ok(
        contrast(readableColor(color, 'light'), 0xf3f5f8) >= MIN_CONTRAST
      );
    }
  });

  test('readable colours are kept', () => {
    assert.equal(readableColor(0xff8000, 'dark'), 0xff8000);
    assert.equal(readableColor(0x1d4ed8, 'light'), 0x1d4ed8);
  });

  test('every 256 colour ends up readable in both themes', () => {
    for (let i = 16; i < 256; i++) {
      assert.ok(
        contrast(readableColor(xterm256(i), 'dark'), 0x2b2d32) >= MIN_CONTRAST
      );
      assert.ok(
        contrast(readableColor(xterm256(i), 'light'), 0xf3f5f8) >= MIN_CONTRAST
      );
    }
  });
});
