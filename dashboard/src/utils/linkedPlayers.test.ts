// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  linkedPlayerInLine,
  linkedPlayers,
  linkedUserOf,
  normalizeUuid,
} from './linkedPlayers.ts';

const linked = linkedPlayers([
  {
    minecraft_uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5',
    minecraft_name: 'Steve',
    username: 'alice',
  },
]);

describe('linked players', () => {
  test('the player list matches by UUID, then by name', () => {
    assert.equal(
      linkedUserOf(linked, {
        name: 'Renamed',
        uuid: '069A79F444E94726A5BEFCA90E38AAF5',
      }),
      'alice'
    );
    assert.equal(linkedUserOf(linked, { name: 'steve', uuid: null }), 'alice');
    assert.equal(linkedUserOf(linked, { name: 'Alex' }), undefined);
    assert.equal(
      normalizeUuid('069a79f444e94726a5befca90e38aaf5'),
      '069a79f4-44e9-4726-a5be-fca90e38aaf5'
    );
  });

  test('console lines about a linked player', () => {
    const user = (line: string) => linkedPlayerInLine(line, linked)?.user;
    assert.equal(user('[12:00:00] [Server thread/INFO]: <Steve> hi'), 'alice');
    assert.equal(
      user('[12:00:00 INFO]: [Not Secure] <Steve> hello there'),
      'alice'
    );
    assert.equal(
      user('[12:00:00] [Server thread/INFO]: Steve joined the game'),
      'alice'
    );
    assert.equal(
      user('[12:00:00] [Server thread/INFO]: Steve left the game'),
      'alice'
    );
    assert.equal(
      user(
        '[12:00:00] [Server thread/INFO]: Steve[/127.0.0.1:5000] logged in with entity id 1'
      ),
      'alice'
    );
    assert.equal(
      user('[12:00:00] [Server thread/INFO]: * Steve waves'),
      'alice'
    );
  });

  test('other lines and other players are not', () => {
    const user = (line: string) => linkedPlayerInLine(line, linked)?.user;
    assert.equal(
      user('[12:00:00] [Server thread/INFO]: <Alex> Steve joined the game'),
      undefined
    );
    assert.equal(
      user('[12:00:00] [Server thread/INFO]: Done (3.2s)!'),
      undefined
    );
    assert.equal(
      user('[12:00:00] [Server thread/INFO]: Steven joined the game'),
      undefined
    );
    assert.equal(linkedPlayerInLine('<Steve> hi', linkedPlayers([])), null);
  });
});
