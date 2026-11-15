// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, mock, test } from 'node:test';
import { DOWNLOAD_FRAME_LIFETIME_MS, startDownload } from './download.ts';

// just enough of a document for startDownload
function fakeDocument() {
  const appended: {
    src: string;
    style: { display: string };
    removed: boolean;
  }[] = [];
  const doc = {
    createElement: () => {
      const frame = {
        src: '',
        style: { display: '' },
        removed: false,
        remove() {
          frame.removed = true;
        },
      };
      return frame;
    },
    body: {
      appendChild: (frame: (typeof appended)[number]) => appended.push(frame),
    },
  };
  return { doc: doc as unknown as Document, appended };
}

describe('startDownload', () => {
  test('loads several links in hidden frames, without popups', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const open = mock.fn();
    const previousWindow = (globalThis as { window?: unknown }).window;
    (globalThis as { window?: unknown }).window = { open };
    try {
      const { doc, appended } = fakeDocument();
      await startDownload('http://core/file/a', doc);
      await startDownload('http://core/file/b', doc);
      assert.deepEqual(
        appended.map((frame) => [frame.src, frame.style.display]),
        [
          ['http://core/file/a', 'none'],
          ['http://core/file/b', 'none'],
        ]
      );
      assert.equal(open.mock.callCount(), 0);

      // the frames go once the downloads have had time to start
      assert.equal(appended[0].removed, false);
      t.mock.timers.tick(DOWNLOAD_FRAME_LIFETIME_MS);
      assert.equal(
        appended.every((frame) => frame.removed),
        true
      );
    } finally {
      (globalThis as { window?: unknown }).window = previousWindow;
    }
  });

  test('hands the link to openUrl when there is one, and reports its failure', async () => {
    const { doc, appended } = fakeDocument();
    const opened: string[] = [];
    await startDownload('http://core/file/a', doc, async (url) =>
      opened.push(url)
    );
    assert.deepEqual(opened, ['http://core/file/a']);
    assert.equal(appended.length, 0);

    await assert.rejects(
      startDownload('http://core/file/b', doc, () =>
        Promise.reject(new Error('denied'))
      ),
      /denied/
    );
  });
});
