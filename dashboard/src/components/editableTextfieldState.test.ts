// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  settleSave,
  type EditableTextfieldState,
} from './editableTextfieldState.ts';

const editing: EditableTextfieldState = {
  displayText: 'Survival',
  editText: 'Survival',
  isEditing: true,
  error: '',
};

describe('settleSave', () => {
  test('a successful save of unchanged text leaves edit mode', () => {
    const next = settleSave(editing, 'Survival', '');
    assert.equal(next.isEditing, false);
    assert.equal(next.displayText, 'Survival');
    assert.equal(next.error, '');
  });

  test('a successful save of changed text commits it and leaves edit mode', () => {
    const next = settleSave(
      { ...editing, editText: 'Creative' },
      'Creative',
      ''
    );
    assert.deepEqual(next, {
      displayText: 'Creative',
      editText: 'Creative',
      isEditing: false,
      error: '',
    });
  });

  test('a successful save clears an earlier error', () => {
    const failed = settleSave(editing, 'Creative', 'name taken');
    const next = settleSave(failed, 'Creative 2', '');
    assert.equal(next.error, '');
    assert.equal(next.isEditing, false);
  });

  test('a rejected save stays in edit mode with the attempted text and error', () => {
    const next = settleSave(
      { ...editing, editText: 'Creative' },
      'Creative',
      'name taken'
    );
    assert.deepEqual(next, {
      displayText: 'Survival',
      editText: 'Creative',
      isEditing: true,
      error: 'name taken',
    });
  });

  test('a rejected save of unchanged text keeps the old display text', () => {
    const next = settleSave(editing, 'Survival', 'offline');
    assert.equal(next.isEditing, true);
    assert.equal(next.displayText, 'Survival');
    assert.equal(next.error, 'offline');
  });
});
